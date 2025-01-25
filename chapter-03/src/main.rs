use crate::core::UrlShortener;
use http_handler::function_handler;
use lambda_http::{
    run, service_fn, tracing, Error,
};
mod core;
mod utils;
mod http_handler;


#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let shortener = UrlShortener::new();

    run(service_fn(|event| function_handler(&shortener, event))).await
}
