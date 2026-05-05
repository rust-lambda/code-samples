//! Tower middleware shape: a layer + service that does literally nothing
//! around its inner service. Useful as a copy-paste skeleton.
//!
//! Run with: `cargo run --example noop_layer`

use std::convert::Infallible;
use std::task::{Context, Poll};

use lambda_http::http::{Request, Response};
use lambda_http::tower::{service_fn, Layer, Service, ServiceBuilder, ServiceExt};
use lambda_http::Body;

#[derive(Clone)]
pub struct NoopLayer;

impl<S> Layer<S> for NoopLayer {
    type Service = NoopService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        NoopService { inner }
    }
}

pub struct NoopService<S> {
    inner: S,
}

impl<S, R> Service<R> for NoopService<S>
where
    S: Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: R) -> Self::Future {
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
    let service = ServiceBuilder::new()
        .layer(NoopLayer)
        .service(service_fn(handler));

    let request = Request::builder()
        .uri("http://example.com/")
        .body(Body::Empty)
        .unwrap();

    let response = service.oneshot(request).await.unwrap();

    println!("status: {}", response.status());
}
