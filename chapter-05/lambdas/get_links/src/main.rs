use lambda_http::{
    http::StatusCode, run, service_fn, tracing, Error, IntoResponse, Request, RequestExt,
};
use shared::core::UrlShortener;
use shared::response::{empty_response, json_response};
use shared::url_info::UrlInfo;
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
    let config = aws_config::load_from_env().await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let http_client = shared::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    let url_info = UrlInfo::new(http_client);
    let shortener = UrlShortener::new(&table_name, dynamodb_client, url_info);

    run(service_fn(|event| function_handler(&shortener, event))).await
}
