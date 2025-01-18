use lambda_http::{
    http::StatusCode, run, service_fn, tracing, Error, IntoResponse, Request, RequestPayloadExt,
};
use shared::core::{ShortenUrlRequest, UrlShortener};
use shared::response::{empty_response, json_response};
use shared::url_info::UrlInfo;
use std::env;

async fn function_handler(
    url_shortener: &UrlShortener,
    url_info: &UrlInfo,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

    let shorten_url_request_body = event.payload::<ShortenUrlRequest>()?;

    match shorten_url_request_body {
        None => empty_response(&StatusCode::BAD_REQUEST),
        Some(shorten_url_request) => {
            let shortened_url_response = url_shortener
                .shorten_url(shorten_url_request, url_info)
                .await;

            match shortened_url_response {
                Ok(response) => json_response(&StatusCode::OK, &response),
                Err(e) => {
                    tracing::error!("Failed to shorten URL: {:?}", e);
                    empty_response(&StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    let table_name = env::var("TABLE_NAME").expect("TABLE_NAME is not set");
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let http_client = shared::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    let url_info = UrlInfo::new(http_client);
    let shortener = UrlShortener::new(&table_name, dynamodb_client);

    run(service_fn(|event| {
        function_handler(&shortener, &url_info, event)
    }))
    .await
}
