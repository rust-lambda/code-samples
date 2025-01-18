use lambda_http::{
    http::StatusCode, run, service_fn, tracing, Error, IntoResponse, Request, RequestExt,
};
use shared::core::UrlShortener;
use shared::response::{empty_response, json_response};
use std::env;

async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

    let query_params = event.query_string_parameters();
    let last_evaluated_id = query_params.first("last_evaluated_id");

    let links = url_shortener.list_urls(last_evaluated_id).await;
    match links {
        Ok(links) => json_response(&StatusCode::OK, &links),
        Err(e) => {
            tracing::error!("Failed to list URLs: {:?}", e);
            empty_response(&StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    let table_name = env::var("TABLE_NAME").expect("TABLE_NAME is not set");
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let shortener = UrlShortener::new(&table_name, dynamodb_client);

    run(service_fn(|event| function_handler(&shortener, event))).await
}
