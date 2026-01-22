use lambda_http::RequestExt;
use lambda_http::{http::StatusCode, tracing, Error, IntoResponse, Request};
use shared::core::UrlRepository;
use shared::utils::{empty_response, json_response};

pub(crate) struct HandlerDeps<R: UrlRepository> {
    pub url_repo: R,
}

#[tracing::instrument(skip(deps, event))]
pub(crate) async fn function_handler<R: UrlRepository>(
    deps: &HandlerDeps<R>,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);
    let query_params = event.query_string_parameters();
    let last_evaluated_id = query_params
        .first("last_evaluated_id")
        .map(|s| s.to_string());

    let links = deps.url_repo.list_urls(last_evaluated_id).await;
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
    use super::{function_handler, HandlerDeps};
    use lambda_http::http::Request;
    use lambda_http::{Body, IntoResponse, RequestExt};
    use mockall::predicate::eq;
    use shared::core::{MockUrlRepository, ShortUrl};
    use std::collections::HashMap;

    #[tokio::test]
    async fn when_valid_request_made_should_return() {
        let mut mock_url_repo = MockUrlRepository::default();
        mock_url_repo
            .expect_list_urls()
            .times(1)
            .with(eq(None))
            .returning(|_last_evaluated_id| {
                Ok((
                    vec![ShortUrl::new(
                        "12345689".into(),
                        "https://google.com".into(),
                    )],
                    None,
                ))
            });
        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap();

        let result = function_handler(&deps, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 200);
    }

    #[tokio::test]
    async fn when_valid_request_made_with_path_parameter_should_return() {
        let mut mock_url_repo = MockUrlRepository::default();
        mock_url_repo
            .expect_list_urls()
            .times(1)
            .with(eq(Some("an-id".to_string()))) // make sure the correct id is propagated
            .returning(|_last_evaluated_id| {
                Ok((
                    vec![ShortUrl::new(
                        "12345689".into(),
                        "https://google.com".into(),
                    )],
                    None,
                ))
            });
        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };
        let mut query_string = HashMap::new();
        query_string.insert("last_evaluated_id".to_string(), "an-id".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_query_string_parameters(query_string);

        let result = function_handler(&deps, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 200);
    }

    #[tokio::test]
    async fn when_error_in_database_return_500() {
        let mut mock_url_repo = MockUrlRepository::default();
        mock_url_repo
            .expect_list_urls()
            .times(1)
            .returning(|_last_evaluated_id| Err("Error reading from DB".to_string()));
        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };
        let mut query_string = HashMap::new();
        query_string.insert("last_evaluated_id".to_string(), "an-id".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_query_string_parameters(query_string);

        let result = function_handler(&deps, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 500);
    }
}
