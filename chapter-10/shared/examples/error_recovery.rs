//! "Bailing out": a middleware that intercepts errors from the inner
//! service and turns them into a structured 503 response. This mirrors
//! the pattern used by `RateLimitLayer::on_unavailable` in the library.
//!
//! Run with: `cargo run --example error_recovery`

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use lambda_http::http::{Request, Response};
use lambda_http::tower::{service_fn, Layer, Service, ServiceBuilder, ServiceExt};
use lambda_http::Body;

/// A trivial application error to demonstrate that the middleware can
/// recover from any inner-service `Err` and produce a successful HTTP
/// response.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("simulated downstream failure")]
    Downstream,
}

#[derive(Clone)]
pub struct RecoverLayer;

impl<S> Layer<S> for RecoverLayer {
    type Service = RecoverService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        RecoverService { inner }
    }
}

pub struct RecoverService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for RecoverService<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = AppError> + Send + 'static,
    S::Future: Send,
{
    type Response = Response<Body>;
    // Surface a runtime-friendly error type that *cannot* happen, so anything
    // that escapes upward into `lambda_http::run` will always be a successful
    // HTTP response.
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Inner readiness errors would also need translation in a real impl;
        // for this demo the inner service is `service_fn` which is always ready.
        self.inner
            .poll_ready(cx)
            .map_err(|_| -> std::convert::Infallible { unreachable!() })
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let fut = self.inner.call(request);
        Box::pin(async move {
            match fut.await {
                Ok(response) => Ok(response),
                Err(error) => {
                    let body = format!(r#"{{"error":"{error}"}}"#);
                    Ok(Response::builder()
                        .status(503)
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap())
                }
            }
        })
    }
}

async fn fallible_handler(_req: Request<Body>) -> Result<Response<Body>, AppError> {
    // Always errors, to exercise the recovery branch.
    Err(AppError::Downstream)
}

#[tokio::main]
async fn main() {
    let service = ServiceBuilder::new()
        .layer(RecoverLayer)
        .service(service_fn(fallible_handler));

    let request = Request::builder()
        .uri("http://example.com/")
        .body(Body::Empty)
        .unwrap();

    let response = service.oneshot(request).await.unwrap();

    println!("status: {}", response.status());
    let body = response.into_body();
    if let Body::Text(s) = body {
        println!("body: {s}");
    }
}
