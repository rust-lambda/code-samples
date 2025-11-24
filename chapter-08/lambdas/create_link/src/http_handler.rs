use crate::event_publisher::EventPublisher;
use lambda_http::RequestPayloadExt;
use lambda_http::{http::StatusCode, tracing, Error, IntoResponse, Request};
use serde::{Deserialize, Serialize};
use shared::core::{IdGenerator, UrlRepository};
use shared::utils::{empty_response, json_response};

#[derive(Serialize, Deserialize)]
pub struct ShortenUrlRequest {
    pub url_to_shorten: String,
}
pub(crate) struct HandlerDeps<I: IdGenerator, R: UrlRepository, E: EventPublisher> {
    pub id_generator: I,
    pub url_repo: R,
    pub event_publisher: E,
}

pub(crate) async fn function_handler<I: IdGenerator, R: UrlRepository, E: EventPublisher>(
    deps: &HandlerDeps<I, R, E>,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

    // TODO: handle bad request in the case the body is not valid JSON or missing fields
    let shorten_url_request_body = event.payload::<ShortenUrlRequest>()?;

    if shorten_url_request_body.is_none() {
        return empty_response(&StatusCode::BAD_REQUEST);
    }
    let url_to_shorten = shorten_url_request_body.unwrap().url_to_shorten;
    let id = deps.id_generator.generate_id();
    let saved = deps.url_repo.store_short_url(url_to_shorten, id).await;
    if let Err(e) = &saved {
        tracing::error!("Failed to shorten URL: {:?}", e);
        return empty_response(&StatusCode::INTERNAL_SERVER_ERROR);
    }
    let short_url = saved.unwrap();
    let publish_result = deps.event_publisher.publish_link_created(&short_url).await;
    if let Err(e) = &publish_result {
        tracing::error!("Failed to publish link created event: {:?}", e);
    }
    json_response(&StatusCode::OK, &short_url)
}

#[cfg(test)]
mod tests {
    use super::function_handler;
    use crate::event_publisher::MockEventPublisher;
    use crate::http_handler::HandlerDeps;
    use lambda_http::http::Request;
    use lambda_http::{Body, IntoResponse};
    use mockall::predicate::{eq, function};
    use serde_json::{json, Value};
    use shared::core::{MockIdGenerator, MockUrlRepository, ShortUrl};

    #[tokio::test]
    async fn when_valid_link_is_passed_should_store_publish_and_return_details() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_id_generator = MockIdGenerator::new();
        mock_id_generator
            .expect_generate_id()
            .times(1)
            .return_const("12345689".to_string());
        mock_url_repo
            .expect_store_short_url()
            .with(
                eq("https://google.com".to_string()),
                eq("12345689".to_string()),
            )
            .times(1)
            .returning(|url_to_shorten, short_link| Ok(ShortUrl::new(short_link, url_to_shorten)));
        let mut event_publisher = MockEventPublisher::new();
        event_publisher
            .expect_publish_link_created()
            .times(1)
            .with(function(|short_url: &ShortUrl| {
                short_url.link_id == "12345689"
            }))
            .returning(|_| Ok(()));
        let deps = HandlerDeps {
            id_generator: mock_id_generator,
            url_repo: mock_url_repo,
            event_publisher,
        };
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(
                json!({"url_to_shorten": "https://google.com"})
                    .to_string()
                    .into(),
            )
            .unwrap();

        let result = function_handler(&deps, request).await;

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
                "title": null,
                "description": null,
                "content_type": null
            })
        );
    }

    #[tokio::test]
    async fn when_invalid_body_is_passed_should_return_400() {
        let mock_url_repo = MockUrlRepository::default();
        let mock_id_generator = MockIdGenerator::new();
        let mut event_publisher = MockEventPublisher::new();
        event_publisher.expect_publish_link_created().times(0);
        let deps = HandlerDeps {
            id_generator: mock_id_generator,
            url_repo: mock_url_repo,
            event_publisher,
        };
        let request = Request::builder().body(Body::Empty).unwrap();

        let result = function_handler(&deps, request).await;

        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 400);
    }

    #[tokio::test]
    async fn when_valid_body_is_passed_and_storage_fails_should_return_500() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_id_generator = MockIdGenerator::new();
        mock_id_generator
            .expect_generate_id()
            .times(1)
            .return_const("12345689".to_string());
        mock_url_repo
            .expect_store_short_url()
            .times(1)
            .returning(|_url_to_shorten, _short_link| Err("Error storing URL".to_string()));
        let mut event_publisher = MockEventPublisher::new();
        event_publisher.expect_publish_link_created().times(0);
        let deps = HandlerDeps {
            id_generator: mock_id_generator,
            url_repo: mock_url_repo,
            event_publisher,
        };
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(
                json!({"url_to_shorten": "https://google.com"})
                    .to_string()
                    .into(),
            )
            .unwrap();

        let result = function_handler(&deps, request).await;

        let data = result.unwrap().into_response().await;
        assert_eq!(data.status(), 500);
    }

    #[tokio::test]
    async fn when_event_publish_fails_should_still_return_200() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_id_generator = MockIdGenerator::new();
        mock_id_generator
            .expect_generate_id()
            .times(1)
            .return_const("short123".to_string());
        mock_url_repo
            .expect_store_short_url()
            .with(
                eq("https://example.com".to_string()),
                eq("short123".to_string()),
            )
            .times(1)
            .returning(|url_to_shorten, short_link| Ok(ShortUrl::new(short_link, url_to_shorten)));
        let mut event_publisher = MockEventPublisher::new();
        event_publisher
            .expect_publish_link_created()
            .times(1)
            .returning(|_| {
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "publish failed",
                )))
            });
        let deps = HandlerDeps {
            id_generator: mock_id_generator,
            url_repo: mock_url_repo,
            event_publisher,
        };
        let request = Request::builder()
            .header("Content-Type", "application/json")
            .body(
                json!({"url_to_shorten": "https://example.com"})
                    .to_string()
                    .into(),
            )
            .unwrap();

        let data = function_handler(&deps, request)
            .await
            .unwrap()
            .into_response()
            .await;

        assert_eq!(data.status(), 200);
    }
}
