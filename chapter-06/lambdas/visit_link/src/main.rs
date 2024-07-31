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

    let link_id = event
        .path_parameters_ref()
        .and_then(|params| params.first("linkId"))
        .unwrap_or("");

    if link_id.is_empty() {
        return generate_api_response(404, "Not Found");
    }

    let full_url = url_shortener
        .retrieve_url_and_increment_clicks(link_id)
        .await;

    match full_url {
        Err(e) => {
            tracing::error!("Failed to retrieve URL: {:?}", e);
            Ok(generate_api_response(500, "Internal Server Error")?)
        }
        Ok(None) => Ok(generate_api_response(404, "Not Found")?),
        Ok(Some(url)) => {
            let response = Response::builder()
                .status(StatusCode::from_u16(302).unwrap())
                .header("Location", url)
                .body("".to_string())
                .map_err(Box::new)?;

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
        return_empty: bool,
    }

    #[async_trait]
    impl UrlRepository for TestUrlRepository {
        async fn get_url_from_short_link(
            &self,
            _short_link: &str,
        ) -> Result<Option<String>, String> {
            if self.return_error {
                Err("Failure storing".to_string())
            } else if self.return_empty {
                Ok(None)
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
    async fn when_valid_request_made_with_path_parameter_should_return_redirect() {
        let mock_url_repo = TestUrlRepository {
            return_error: false,
            return_empty: false
        };
        let test_url_info = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mock_url_repo), Box::new(test_url_info));

        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "aoinf87".to_string());

        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&url_shortener, request).await;

        assert_eq!(result.is_ok(), true);

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 302);
    }

    #[tokio::test]
    async fn when_link_id_not_passed_should_return_404() {
        let mock_url_repo = TestUrlRepository {
            return_error: false,
            return_empty: false
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

        assert_eq!(data.status(), 404);
    }

    #[tokio::test]
    async fn when_database_errors_should_return_500() {
        let mock_url_repo = TestUrlRepository {
            return_error: true,
            return_empty: false
        };
        let test_url_info = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mock_url_repo), Box::new(test_url_info));

        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "aoinf87".to_string());

        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&url_shortener, request).await;

        assert_eq!(result.is_ok(), true);

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 500);
    }

    #[tokio::test]
    async fn when_database_not_found_should_return_404() {
        let mock_url_repo = TestUrlRepository {
            return_error: false,
            return_empty: true
        };
        let test_url_info = TestUrlInfo {};

        let url_shortener = UrlShortener::new(Box::new(mock_url_repo), Box::new(test_url_info));

        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "aoinf87".to_string());

        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&url_shortener, request).await;

        assert_eq!(result.is_ok(), true);

        let data = result.unwrap().into_response().await;

        assert_eq!(data.status(), 404);
    }
}
