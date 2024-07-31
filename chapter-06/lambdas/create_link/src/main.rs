use lambda_http::{run, service_fn, tracing, Error, IntoResponse, Request, RequestPayloadExt};
use shared::adapters::DynamoDbUrlRepository;
use shared::core::{ShortenUrlRequest, UrlShortener};
use shared::url_info::HttpUrlInfo;
use shared::utils::generate_api_response;
use std::env;

async fn function_handler(
    url_shortener: &UrlShortener,
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
    let shortener = UrlShortener::new(Box::new(url_repo), Box::new(url_info));

    run(service_fn(|event| function_handler(&shortener, event))).await
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use lambda_http::http::Request;
    use lambda_http::Body;
    use lambda_http::IntoResponse;
    use serde_json::Value;
    use shared::core::ShortUrl;
    use shared::core::UrlInfo;
    use shared::core::UrlRepository;
    use shared::core::UrlShortener;
    use shared::url_info::UrlDetails;

    use crate::function_handler;

    #[derive(Debug)]
    struct TestUrlRepository {
        return_error: bool,
    }

    #[async_trait]
    impl UrlRepository for TestUrlRepository {
        async fn get_url_from_short_link(
            &self,
            short_link: &str,
        ) -> Result<Option<String>, String> {
            Ok(Some("https://google.com".to_string()))
        }

        async fn store_short_url(
            &self,
            url_to_shorten: String,
            short_url: String,
            url_details: UrlDetails,
        ) -> Result<ShortUrl, String> {
            if (self.return_error) {
                Err("Failure storing".to_string())
            } else {
                Ok(ShortUrl::new(
                    short_url,
                    url_to_shorten,
                    0,
                    url_details.title,
                    url_details.description,
                    url_details.content_type,
                ))
            }
        }

        async fn list_urls(
            &self,
            last_evaluated_id: Option<String>,
        ) -> Result<(Vec<ShortUrl>, Option<String>), String> {
            Ok((
                vec![ShortUrl::new(
                    "12345689".to_string(),
                    "https://google.com".to_string(),
                    0,
                    None,
                    None,
                    None,
                )],
                None,
            ))
        }
    }

    #[derive(Debug)]
    struct TestUrlInfo {}

    #[async_trait]
    impl UrlInfo for TestUrlInfo {
        async fn fetch_details(&self, url: &str) -> Result<UrlDetails, String> {
            Ok(UrlDetails {
                content_type: Some("text/html".to_string()),
                title: Some("Google".to_string()),
                description: Some("Test description".to_string()),
            })
        }
    }

    #[tokio::test]
    async fn when_valid_link_is_passed_should_store_and_return_details() {
        let mockUrlRepo = TestUrlRepository {
            return_error: false,
        };
        let testUrlInfo = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mockUrlRepo), Box::new(testUrlInfo));

        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::from("{\"url_to_shorten\": \"https://google.com\"}"))
            .unwrap();

        let result = function_handler(&url_shortener, request).await;

        assert_eq!(result.is_ok(), true);

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 200);

        let response_struct: Value = serde_json::from_slice(data.body()).unwrap();

        assert_eq!(response_struct["original_link"], "https://google.com");
        assert_eq!(response_struct["clicks"], 0);
    }

    #[tokio::test]
    async fn when_invalid_body_is_passed_should_return_400() {
        let mockUrlRepo = TestUrlRepository {
            return_error: false,
        };
        let testUrlInfo = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mockUrlRepo), Box::new(testUrlInfo));

        let request = Request::builder().body(Body::Empty).unwrap();

        let result = function_handler(&url_shortener, request).await;

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 400);
    }

    #[tokio::test]
    async fn when_valid_body_is_passed_and_storage_fails_should_return_500() {
        let mockUrlRepo = TestUrlRepository { return_error: true };
        let testUrlInfo = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mockUrlRepo), Box::new(testUrlInfo));

        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::from("{\"url_to_shorten\": \"https://google.com\"}"))
            .unwrap();

        let result = function_handler(&url_shortener, request).await;

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 500);
    }
}
