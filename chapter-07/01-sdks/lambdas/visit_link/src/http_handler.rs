use lambda_http::RequestExt;
use lambda_http::{http::StatusCode, tracing, Error, IntoResponse, Request};
use shared::core::{UrlInfo, UrlRepository, UrlShortener};
use shared::utils::{empty_response, redirect_response};

pub(crate) async fn function_handler<R: UrlRepository, I: UrlInfo>(
    url_shortener: &UrlShortener<R, I>,
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
    use shared::core::UrlShortener;
    use std::collections::HashMap;

    #[tokio::test]
    async fn when_valid_request_made_with_path_parameter_should_return_redirect() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        mock_url_repo
            .expect_get_url_from_short_link()
            .times(1)
            .with(predicate::eq("aoinf87".to_string())) // make sure the correct id is propagated
            .returning(|_link_id| Ok(Some("https://google.com".to_string())));
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "aoinf87".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 302);
    }

    #[tokio::test]
    async fn when_link_id_not_passed_should_return_404() {
        let mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap();

        let result = function_handler(&url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 404);
    }

    #[tokio::test]
    async fn when_database_errors_should_return_500() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        mock_url_repo
            .expect_get_url_from_short_link()
            .times(1)
            .with(predicate::eq("aoinf87".to_string()))
            .returning(|_link_id| Err("Failed to retrieve from DB".to_string()));
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);
        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "aoinf87".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 500);
    }

    #[tokio::test]
    async fn when_link_not_found_should_return_404() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mock_url_info = MockUrlInfo::default();
        mock_url_repo
            .expect_get_url_from_short_link()
            .times(1)
            .with(predicate::eq("aoinf87".to_string()))
            .returning(|_link_id| Ok(None));
        let url_shortener = UrlShortener::new(mock_url_repo, mock_url_info);

        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "aoinf87".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&url_shortener, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 404);
    }
}
