use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use lambda_http::http::{Request, Response};
use lambda_http::tower::{Layer, Service};
use lambda_http::{Body, IntoResponse};
use tracing::Instrument;

use crate::observability::OtelGuard;

const OTEL_STATUS_OK: &str = "OK";
const OTEL_STATUS_ERROR: &str = "ERROR";

pub trait TelemetryFlusher: Send + Sync {
    fn flush(&self);
}

impl TelemetryFlusher for OtelGuard {
    fn flush(&self) {
        OtelGuard::flush(self);
    }
}

#[derive(Clone)]
pub struct HttpObservabilityLayer {
    flusher: Arc<dyn TelemetryFlusher>,
    cold_start: Arc<AtomicBool>,
}

impl HttpObservabilityLayer {
    pub fn new(otel_guard: Arc<OtelGuard>) -> Self {
        Self::with_flusher(otel_guard)
    }

    pub fn with_flusher(flusher: Arc<dyn TelemetryFlusher>) -> Self {
        Self {
            flusher,
            cold_start: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl<S> Layer<S> for HttpObservabilityLayer {
    type Service = HttpObservabilityService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpObservabilityService {
            inner,
            flusher: Arc::clone(&self.flusher),
            cold_start: Arc::clone(&self.cold_start),
        }
    }
}

#[derive(Clone)]
pub struct HttpObservabilityService<S> {
    inner: S,
    flusher: Arc<dyn TelemetryFlusher>,
    cold_start: Arc<AtomicBool>,
}

impl<S> Service<Request<Body>> for HttpObservabilityService<S>
where
    S: Service<Request<Body>> + Send + 'static,
    S::Response: IntoResponse + Send,
    S::Error: Send,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let was_cold_start = self.cold_start.swap(false, Ordering::SeqCst);
        let method = request.method().as_str().to_owned();
        let path = request.uri().path().to_owned();
        let flusher = Arc::clone(&self.flusher);

        let span = tracing::info_span!(
            "aws.lambda",
            operation_name = "aws.lambda",
            faas.coldstart = was_cold_start,
            cloud.provider = "aws",
            event_type = "http",
            http.request.method = %method,
            url.path = %path,
            http.response.status_code = tracing::field::Empty,
            otel.status_code = tracing::field::Empty,
        );

        let inner_future = self.inner.call(request);

        Box::pin(async move {
            let result = inner_future.instrument(span.clone()).await;

            match result {
                Ok(inner_response) => {
                    let response = inner_response.into_response().await;
                    let status = response.status();
                    span.record("http.response.status_code", status.as_u16());

                    if status.is_server_error() {
                        span.record("otel.status_code", OTEL_STATUS_ERROR);
                    } else {
                        span.record("otel.status_code", OTEL_STATUS_OK);
                    }

                    flusher.flush();
                    Ok(response)
                }
                Err(error) => {
                    span.record("otel.status_code", OTEL_STATUS_ERROR);
                    flusher.flush();
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_http::http::StatusCode;
    use lambda_http::tower::{service_fn, ServiceBuilder, ServiceExt};
    use lambda_http::Error;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct CountingFlusher {
        calls: AtomicUsize,
    }

    impl CountingFlusher {
        fn count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl TelemetryFlusher for CountingFlusher {
        fn flush(&self) {
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    async fn ok_handler(_req: Request<Body>) -> Result<Response<Body>, Error> {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Body::from("ok"))
            .unwrap())
    }

    async fn err_handler(_req: Request<Body>) -> Result<Response<Body>, Error> {
        Err("boom".into())
    }

    #[tokio::test]
    async fn flushes_telemetry_after_success() {
        let flusher = Arc::new(CountingFlusher::default());
        let layer = HttpObservabilityLayer::with_flusher(flusher.clone());

        let service = ServiceBuilder::new()
            .layer(layer)
            .service(service_fn(ok_handler));

        let request = Request::builder()
            .method("POST")
            .uri("https://example.com/links")
            .body(Body::Empty)
            .unwrap();

        let response = service.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(flusher.count(), 1);
    }

    #[tokio::test]
    async fn flushes_telemetry_after_inner_error() {
        let flusher = Arc::new(CountingFlusher::default());
        let layer = HttpObservabilityLayer::with_flusher(flusher.clone());

        let service = ServiceBuilder::new()
            .layer(layer)
            .service(service_fn(err_handler));

        let request = Request::builder()
            .method("GET")
            .uri("https://example.com/links")
            .body(Body::Empty)
            .unwrap();

        let result = service.oneshot(request).await;

        assert!(result.is_err());
        assert_eq!(flusher.count(), 1);
    }

    #[tokio::test]
    async fn tracks_cold_start_across_invocations() {
        let flusher = Arc::new(CountingFlusher::default());
        let layer = HttpObservabilityLayer::with_flusher(flusher.clone());

        assert!(layer.cold_start.load(Ordering::SeqCst));

        let service = ServiceBuilder::new()
            .layer(layer.clone())
            .service(service_fn(ok_handler));
        let first_req = Request::builder()
            .uri("https://example.com/links")
            .body(Body::Empty)
            .unwrap();
        let _ = service.oneshot(first_req).await.unwrap();

        assert!(!layer.cold_start.load(Ordering::SeqCst));

        let service = ServiceBuilder::new()
            .layer(layer)
            .service(service_fn(ok_handler));
        let second_req = Request::builder()
            .uri("https://example.com/links")
            .body(Body::Empty)
            .unwrap();
        let _ = service.oneshot(second_req).await.unwrap();

        assert_eq!(flusher.count(), 2);
    }

    #[tokio::test]
    async fn flushes_telemetry_after_short_circuited_response() {
        let flusher = Arc::new(CountingFlusher::default());
        let layer = HttpObservabilityLayer::with_flusher(flusher.clone());

        let service =
            ServiceBuilder::new()
                .layer(layer)
                .service(service_fn(|_req: Request<Body>| async {
                    Ok::<_, Error>(
                        Response::builder()
                            .status(StatusCode::TOO_MANY_REQUESTS)
                            .body(Body::from(r#"{"error":"rate limit exceeded"}"#))
                            .unwrap(),
                    )
                }));

        let request = Request::builder()
            .method("POST")
            .uri("https://example.com/links")
            .body(Body::Empty)
            .unwrap();

        let response = service.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(flusher.count(), 1);
    }
}
