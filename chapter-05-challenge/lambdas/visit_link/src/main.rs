use lambda_http::{
    http::StatusCode, run, service_fn, tracing, Error, IntoResponse, Request, RequestExt,
};
use shared::core::UrlShortener;
use shared::response::{empty_response, redirect_response};
use std::env;

async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

    let link_id = event
        .path_parameters_ref()
        .and_then(|params| params.first("linkId"))
        .unwrap_or("");

    if link_id.is_empty() {
        return empty_response(&StatusCode::NOT_FOUND);
    }

    let full_url = url_shortener
        .retrieve_url_and_increment_clicks(link_id)
        .await;

    match full_url {
        Err(e) => {
            tracing::error!("Failed to retrieve URL: {:?}", e);
            empty_response(&StatusCode::INTERNAL_SERVER_ERROR)
        }
        Ok(None) => empty_response(&StatusCode::NOT_FOUND),
        Ok(Some(url)) => redirect_response(&url),
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
