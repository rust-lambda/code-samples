use lambda_http::RequestExt;
use lambda_http::{http::StatusCode, tracing, Error, IntoResponse, Request};
use shared::configuration::Config;
use shared::core::{UrlInfo, UrlRepository, UrlShortener};
use shared::utils::{empty_response, json_response};

pub(crate) async fn function_handler<R: UrlRepository, I: UrlInfo, T: Config>(
    configuration: &T,
    url_shortener: &UrlShortener<R, I>,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);
    let config = configuration.refresh().await;

    let query_params = event.query_string_parameters();
    let last_evaluated_id = query_params
        .first("last_evaluated_id")
        .map(|s| s.to_string());

    let links = url_shortener.list_urls(&config, last_evaluated_id).await;
    match links {
        Ok(links) => json_response(&StatusCode::OK, &links),
        Err(e) => {
            tracing::error!("Failed to list URLs: {:?}", e);
            empty_response(&StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::function_handler;
    use lambda_http::http::Request;
    use lambda_http::Body;
    use lambda_http::IntoResponse;
    use lambda_http::RequestExt;
    use mockall::predicate;
    use shared::configuration::Configuration;
    use shared::configuration::MockConfig;
    use shared::core::MockUrlInfo;
    use shared::core::MockUrlRepository;
    use shared::core::ShortUrl;
    use shared::core::UrlShortener;
    use std::collections::HashMap;

    #[tokio::test]
    async fn when_valid_request_made_should_return() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        let mut mock_configuration_manager = MockConfig::default();
        mock_configuration_manager
            .expect_refresh()
            .times(1)
            .returning(|| Configuration::default());

        mock_url_repo
            .expect_list_urls()
            .times(1)
            .returning(|_, _last_evaluated_id| {
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

        let result = function_handler(&mock_configuration_manager, &url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 200);
    }

    #[tokio::test]
    async fn when_valid_request_made_with_path_parameter_should_return() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        let mut mock_configuration_manager = MockConfig::default();
        mock_configuration_manager
            .expect_refresh()
            .times(1)
            .returning(|| Configuration::default());

        mock_url_repo
            .expect_list_urls()
            .times(1)
            .with(
                predicate::always(),
                predicate::eq(Some("an-id".to_string())),
            ) // make sure the correct id is propagated
            .returning(|_, _last_evaluated_id| {
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

        let result = function_handler(&mock_configuration_manager, &url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 200);
    }

    #[tokio::test]
    async fn when_error_in_database_return_500() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        let mut mock_configuration_manager = MockConfig::default();
        mock_configuration_manager
            .expect_refresh()
            .times(1)
            .returning(|| Configuration::default());

        mock_url_repo
            .expect_list_urls()
            .times(1)
            .returning(|_, _last_evaluated_id| Err("Error reading from DB".to_string()));
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let mut query_string = HashMap::new();
        query_string.insert("last_evaluated_id".to_string(), "an-id".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_query_string_parameters(query_string);

        let result = function_handler(&mock_configuration_manager, &url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 500);
    }
}
