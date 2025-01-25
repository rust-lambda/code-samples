use http_handler::function_handler;
use lambda_http::{run, service_fn, tracing, Error};
use shared::core::UrlShortener;
use std::env;

mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    let table_name = env::var("TABLE_NAME").expect("TABLE_NAME is not set");
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let shortener = UrlShortener::new(&table_name, dynamodb_client);

    run(service_fn(|event| function_handler(&shortener, event))).await
}
