use lambda_http::{
    http::StatusCode, run, service_fn, tracing, Error, IntoResponse, Request, RequestExt,
};
use shared::adapters::DynamoDbUrlRepository;
use shared::core::{UrlInfo, UrlRepository, UrlShortener};
use shared::url_info::HttpUrlInfo;
use shared::utils::{empty_response, json_response};
use std::env;

async fn function_handler<R: UrlRepository, I: UrlInfo>(
    url_shortener: &UrlShortener<R, I>,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

    let query_params = event.query_string_parameters();
    let last_evaluated_id = query_params
        .first("last_evaluated_id")
        .map(|s| s.to_string());

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
    use lambda_http::RequestExt;
    use mockall::predicate;
    use shared::core::MockUrlInfo;
    use shared::core::MockUrlRepository;
    use shared::core::ShortUrl;
    use shared::core::UrlShortener;
    use std::collections::HashMap;

    #[tokio::test]
    async fn when_valid_request_made_should_return() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        mock_url_repo
            .expect_list_urls()
            .times(1)
            .returning(|_last_evaluated_id| {
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
            });
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap();

        let result = function_handler(&url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 200);
    }

    #[tokio::test]
    async fn when_valid_request_made_with_path_parameter_should_return() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        mock_url_repo
            .expect_list_urls()
            .times(1)
            .with(predicate::eq(Some("an-id".to_string()))) // make sure the correct id is propagated
            .returning(|_last_evaluated_id| {
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
            });
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let mut query_string = HashMap::new();
        query_string.insert("last_evaluated_id".to_string(), "an-id".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_query_string_parameters(query_string);

        let result = function_handler(&url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 200);
    }

    #[tokio::test]
    async fn when_error_in_database_return_500() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        mock_url_repo
            .expect_list_urls()
            .times(1)
            .returning(|_last_evaluated_id| Err("Error reading from DB".to_string()));
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let mut query_string = HashMap::new();
        query_string.insert("last_evaluated_id".to_string(), "an-id".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_query_string_parameters(query_string);

        let result = function_handler(&url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 500);
    }
}
