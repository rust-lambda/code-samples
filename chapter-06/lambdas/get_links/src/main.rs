use lambda_http::http::StatusCode;
use lambda_http::{run, service_fn, tracing, Error, IntoResponse, Request, RequestExt, Response};
use shared::adapters::DynamoDbUrlRepository;
use shared::core::UrlShortener;
use shared::url_info::HttpUrlInfo;
use shared::utils::generate_api_response;
use std::env;

async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

    let query_params = event.query_string_parameters();
    let last_evaluated_id = query_params
        .first("last_evaluated_id")
        .map(|s| s.to_string());

    let links = url_shortener.list_urls(last_evaluated_id).await;
    match links {
        Ok(links) => {
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(serde_json::to_string(&links)?)
                .map_err(Box::new)?;
            Ok(response)
        }
        Err(e) => {
            tracing::error!("Failed to list URLs: {:?}", e);
            Ok(generate_api_response(500, "Internal Server Error")?)
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
    use lambda_http::RequestExt;
    use shared::core::ShortUrl;
    use shared::core::UrlInfo;
    use shared::core::UrlRepository;
    use shared::core::UrlShortener;
    use shared::url_info::UrlDetails;
    use std::collections::HashMap;

    use crate::function_handler;

    #[derive(Debug)]
    struct TestUrlRepository {
        return_error: bool,
    }

    #[async_trait]
    impl UrlRepository for TestUrlRepository {
        async fn get_url_from_short_link(
            &self,
            _short_link: &str,
        ) -> Result<Option<String>, String> {
            if self.return_error {
                Err("Failure storing".to_string())
            } else {
                Ok(Some("https://google.com".to_string()))
            }
        }

        async fn store_short_url(
            &self,
            url_to_shorten: String,
            short_url: String,
            url_details: UrlDetails,
        ) -> Result<ShortUrl, String> {
            if self.return_error {
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
            _last_evaluated_id: Option<String>,
        ) -> Result<(Vec<ShortUrl>, Option<String>), String> {
            if self.return_error {
                Err("Failure storing".to_string())
            } else {
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
    async fn when_valid_request_made_should_return() {
        let mock_url_repo = TestUrlRepository {
            return_error: false,
        };
        let test_url_info = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mock_url_repo), Box::new(test_url_info));

        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap();

        let result = function_handler(&url_shortener, request).await;

        assert_eq!(result.is_ok(), true);

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 200);
    }

    #[tokio::test]
    async fn when_valid_request_made_with_path_parameter_should_return() {
        let mock_url_repo = TestUrlRepository {
            return_error: false,
        };
        let test_url_info = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mock_url_repo), Box::new(test_url_info));

        let mut query_string = HashMap::new();
        query_string.insert("last_evaluated_id".to_string(), "an-id".to_string());

        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_query_string_parameters(query_string);

        let result = function_handler(&url_shortener, request).await;

        assert_eq!(result.is_ok(), true);

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 200);
    }

    #[tokio::test]
    async fn when_error_in_database_return_500() {
        let mock_url_repo = TestUrlRepository { return_error: true };
        let test_url_info = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mock_url_repo), Box::new(test_url_info));

        let mut query_string = HashMap::new();
        query_string.insert("last_evaluated_id".to_string(), "an-id".to_string());

        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_query_string_parameters(query_string);

        let result = function_handler(&url_shortener, request).await;

        assert_eq!(result.is_ok(), true);

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 500);
    }
}
