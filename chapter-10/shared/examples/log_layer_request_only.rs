//! examples/log_layer_request_only.rs
//!
//! Stage 1 of the log layer evolution: the no-op middleware shape with one
//! extra `tracing::info!` line in `call`. We log the HTTP method and path
//! on the way *in*, then delegate to the inner service untouched.
//!
//! `type Future = S::Future` is unchanged from the no-op, so there is no
//! boxing, no `async move`, no extra trait bounds, and no new types. The
//! whole change vs `examples/noop_layer.rs` is one log statement in `call`.
//!
//! The next question (and the next stage) is: what if we want the response
//! status in the log line too? See `examples/log_layer.rs` for the
//! boxed-future version.
//!
//! Run with: `cargo run --example log_layer_request_only`

use std::convert::Infallible;
use std::task::{Context, Poll};

use lambda_http::http::{Request, Response};
use lambda_http::tower::{service_fn, Layer, Service, ServiceBuilder, ServiceExt};
use lambda_http::{tracing, Body};

#[derive(Clone)]
pub struct LogLayer;

impl<S> Layer<S> for LogLayer {
    type Service = LogService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        LogService { inner }
    }
}

pub struct LogService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for LogService<S>
where
    S: Service<Request<Body>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        tracing::info!(
            method = %request.method(),
            path = %request.uri().path(),
            "request"
        );
        self.inner.call(request)
    }
}

async fn handler(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::builder()
        .status(200)
        .body(Body::from(r#"{"message":"hello"}"#))
        .unwrap())
}

#[tokio::main]
async fn main() {
    tracing::init_default_subscriber();

    let service = ServiceBuilder::new()
        .layer(LogLayer)
        .service(service_fn(handler));

    let request = Request::builder()
        .method("GET")
        .uri("http://example.com/hello")
        .body(Body::Empty)
        .unwrap();

    let response = service.oneshot(request).await.unwrap();

    println!("status: {}", response.status());
}
