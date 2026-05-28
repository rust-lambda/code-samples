//! A middleware that injects an `x-powered-by: rust` header into every
//! outgoing response.
//!
//! Run with: `cargo run --example powered_by_layer`

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use lambda_http::http::{HeaderValue, Request, Response};
use lambda_http::tower::{service_fn, Layer, Service, ServiceBuilder, ServiceExt};
use lambda_http::Body;

#[derive(Clone)]
pub struct PoweredByLayer;

impl<S> Layer<S> for PoweredByLayer {
    type Service = PoweredByService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        PoweredByService { inner }
    }
}

pub struct PoweredByService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for PoweredByService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Send + 'static,
    S::Future: Send,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let fut = self.inner.call(request);
        Box::pin(async move {
            let mut response = fut.await?;
            response
                .headers_mut()
                .insert("x-powered-by", HeaderValue::from_static("rust"));
            Ok(response)
        })
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
        .layer(PoweredByLayer)
        .service(service_fn(handler));

    let request = Request::builder()
        .uri("http://example.com/")
        .body(Body::Empty)
        .unwrap();

    let response = service.oneshot(request).await.unwrap();

    println!("status: {}", response.status());
    println!("x-powered-by: {:?}", response.headers().get("x-powered-by"));
}
