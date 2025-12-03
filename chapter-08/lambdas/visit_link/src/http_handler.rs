use crate::event_publisher::EventPublisher;
use lambda_http::RequestExt;
use lambda_http::{http::StatusCode, tracing, Error, IntoResponse, Request};
use shared::core::UrlRepository;
use shared::utils::{empty_response, redirect_response};

pub(crate) struct HandlerDeps<R: UrlRepository, E: EventPublisher> {
    pub url_repo: R,
    pub event_publisher: E,
}

pub(crate) async fn function_handler<R: UrlRepository, E: EventPublisher>(
    deps: &HandlerDeps<R, E>,
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

    let full_url = deps.url_repo.get_url_from_short_link(link_id).await;

    match full_url {
        Err(e) => {
            tracing::error!("Failed to retrieve URL: {:?}", e);
            empty_response(&StatusCode::INTERNAL_SERVER_ERROR)
        }
        Ok(None) => empty_response(&StatusCode::NOT_FOUND),
        Ok(Some(short_url)) => {
            let publish_result = deps.event_publisher.publish_link_clicked(&short_url).await;
            if let Err(e) = &publish_result {
                tracing::warn!("Failed to publish link clicked event: {:?}", e);
            }
            redirect_response(&short_url.original_link)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{function_handler, HandlerDeps};
    use crate::event_publisher::MockEventPublisher;
    use lambda_http::http::Request;
    use lambda_http::{Body, IntoResponse, RequestExt};
    use mockall::predicate::{eq, function};
    use shared::core::{MockUrlRepository, ShortUrl};
    use std::collections::HashMap;

    #[tokio::test]
    async fn when_valid_request_made_with_path_parameter_should_return_redirect() {
        let mut mock_url_repo = MockUrlRepository::default();
        mock_url_repo
            .expect_get_url_from_short_link()
            .times(1)
            .with(eq("123456789".to_string())) // make sure the correct id is propagated
            .returning(|link_id| {
                Ok(Some(ShortUrl::new(
                    link_id.to_string(),
                    "https://google.com".into(),
                )))
            });
        let mut event_publisher = MockEventPublisher::new();
        event_publisher
            .expect_publish_link_clicked()
            .times(1)
            .with(function(|short_url: &ShortUrl| {
                short_url.link_id == "123456789"
            }))
            .returning(|_| Ok(()));
        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            event_publisher,
        };
        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "123456789".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&deps, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 302);
    }

    #[tokio::test]
    async fn when_link_id_not_passed_should_return_404() {
        let mut mock_url_repo = MockUrlRepository::default();
        mock_url_repo.expect_get_url_from_short_link().times(0);
        let mut event_publisher = MockEventPublisher::new();
        event_publisher.expect_publish_link_clicked().times(0);
        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            event_publisher,
        };
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap();

        let result = function_handler(&deps, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 404);
    }

    #[tokio::test]
    async fn when_database_errors_should_return_500() {
        let mut mock_url_repo = MockUrlRepository::default();
        mock_url_repo
            .expect_get_url_from_short_link()
            .times(1)
            .with(eq("aoinf87".to_string()))
            .returning(|_link_id| Err("Failed to retrieve from DB".to_string()));
        let mut event_publisher = MockEventPublisher::new();
        event_publisher.expect_publish_link_clicked().times(0);
        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            event_publisher,
        };
        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "aoinf87".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&deps, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 500);
    }

    #[tokio::test]
    async fn when_link_not_found_should_return_404() {
        let mut mock_url_repo = MockUrlRepository::default();
        mock_url_repo
            .expect_get_url_from_short_link()
            .times(1)
            .with(eq("aoinf87".to_string()))
            .returning(|_link_id| Ok(None));
        let mut event_publisher = MockEventPublisher::new();
        event_publisher.expect_publish_link_clicked().times(0);
        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            event_publisher,
        };

        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "aoinf87".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let result = function_handler(&deps, request).await;

        assert!(result.is_ok());
        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 404);
    }

    #[tokio::test]
    async fn when_publish_fails_should_still_redirect() {
        let mut mock_url_repo = MockUrlRepository::default();
        mock_url_repo
            .expect_get_url_from_short_link()
            .times(1)
            .with(eq("abc123".to_string()))
            .returning(|link_id| {
                Ok(Some(ShortUrl::new(
                    link_id.to_string(),
                    "https://example.com".into(),
                )))
            });
        let mut event_publisher = MockEventPublisher::new();
        event_publisher
            .expect_publish_link_clicked()
            .times(1)
            .returning(|_| {
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "publish failed",
                )))
            });
        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            event_publisher,
        };
        let mut path_params = HashMap::new();
        path_params.insert("linkId".to_string(), "abc123".to_string());
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(Body::Empty)
            .unwrap()
            .with_path_parameters(path_params);

        let response = function_handler(&deps, request)
            .await
            .unwrap()
            .into_response()
            .await;

        assert_eq!(response.status(), 302);
    }
}
