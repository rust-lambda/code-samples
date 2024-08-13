use lambda_http::{run, service_fn, tracing, Error, IntoResponse, Request, RequestPayloadExt};
use shared::adapters::DynamoDbUrlRepository;
use shared::core::{ShortenUrlRequest, UrlInfo, UrlRepository, UrlShortener};
use shared::url_info::HttpUrlInfo;
use shared::utils::generate_api_response;
use std::env;

async fn function_handler<R: UrlRepository, I: UrlInfo>(
    url_shortener: &UrlShortener<R, I>,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

    let shorten_url_request_body = event.payload::<ShortenUrlRequest>()?;

    match shorten_url_request_body {
        None => generate_api_response(400, "Bad request"),
        Some(shorten_url_request) => {
            let shortened_url_response = url_shortener.shorten_url(shorten_url_request).await;

            let response = match shortened_url_response {
                Ok(response) => {
                    generate_api_response(200, &serde_json::to_string(&response).unwrap())?
                }
                Err(e) => {
                    tracing::error!("Failed to shorten URL: {:?}", e);
                    generate_api_response(500, "Internal Server Error")?
                }
            };

            Ok(response)
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
    let url_info = HttpUrlInfo::new(http_client);
    let url_repo = DynamoDbUrlRepository::new(table_name, dynamodb_client);
    let shortener = UrlShortener::new(url_repo, url_info);

    run(service_fn(|event| function_handler(&shortener, event))).await
}

#[cfg(test)]
mod tests {
    use crate::function_handler;
    use lambda_http::http::Request;
    use lambda_http::Body;
    use lambda_http::IntoResponse;
    use serde_json::{json, Value};
    use shared::core::MockUrlInfo;
    use shared::core::MockUrlRepository;
    use shared::core::ShortUrl;
    use shared::core::UrlShortener;
    use shared::url_info::UrlDetails;

    #[tokio::test]
    async fn when_valid_link_is_passed_should_store_and_return_details() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_url_info = MockUrlInfo::default();
        mock_url_repo.expect_store_short_url().times(1).returning(
            |url_to_shorten, _short_url, url_details| {
                Ok(ShortUrl::new(
                    "12345689".to_string(),
                    url_to_shorten,
                    0,
                    url_details.title,
                    url_details.description,
                    url_details.content_type,
                ))
            },
        );
        mock_url_info
            .expect_fetch_details()
            .times(1)
            .returning(|_url| {
                Ok(UrlDetails {
                    content_type: Some("text/html".to_string()),
                    title: Some("Google".to_string()),
                    description: Some("Test description".to_string()),
                })
            });
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(
                json!({"url_to_shorten": "https://google.com"})
                    .to_string()
                    .into(),
            )
            .unwrap();

        let result = function_handler(&url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 200);
        let response_struct: Value = serde_json::from_slice(data.body()).unwrap();
        assert_eq!(
            response_struct,
            json!({
                "link_id": "12345689",
                "original_link": "https://google.com",
                "clicks": 0,
                "title": "Google",
                "description": "Test description",
                "content_type": "text/html"
            })
        );
    }

    #[tokio::test]
    async fn when_invalid_body_is_passed_should_return_400() {
        let mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let request = Request::builder().body(Body::Empty).unwrap();

        let result = function_handler(&url_shortener, request).await;

        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 400);
    }

    #[tokio::test]
    async fn when_valid_body_is_passed_and_storage_fails_should_return_500() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_url_info = MockUrlInfo::default();
        mock_url_repo.expect_store_short_url().times(1).returning(
            |_url_to_shorten, _short_url, _url_details| Err("Error storing URL".to_string()),
        );
        mock_url_info
            .expect_fetch_details()
            .times(1)
            .returning(|_url| {
                Ok(UrlDetails {
                    content_type: Some("text/html".to_string()),
                    title: Some("Google".to_string()),
                    description: Some("Test description".to_string()),
                })
            });
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(
                json!({"url_to_shorten": "https://google.com"})
                    .to_string()
                    .into(),
            )
            .unwrap();

        let result = function_handler(&url_shortener, request).await;

        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 500);
    }
}
