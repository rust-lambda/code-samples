use aws_lambda_events::{
    event::sqs::SqsEvent,
    sqs::{SqsBatchResponse, SqsMessage},
};
use cloudevents::AttributesReader;
use lambda_runtime::{tracing, Error, LambdaEvent};
use opentelemetry::global;
use shared::{
    core::{ShortUrl, UrlInfo, UrlRepository},
    observability::add_span_link_from,
};

pub(crate) struct HandlerDeps<R: UrlRepository, I: UrlInfo> {
    pub url_repo: R,
    pub url_info: I,
}

#[tracing::instrument(skip(deps, event))]
pub(crate) async fn function_handler<R: UrlRepository, I: UrlInfo>(
    deps: &HandlerDeps<R, I>,
    event: LambdaEvent<SqsEvent>,
) -> Result<SqsBatchResponse, Error> {
    let meter = global::meter("process_link_created");
    let link_created_counter = meter.u64_counter("links_created").build();

    let mut sqs_batch_response = SqsBatchResponse::default();
    let payload = event.clone().payload;
    let batch_count = payload.records.len();

    let tasks: Vec<_> = event
        .payload
        .records
        .into_iter()
        .map(|message| process_message(&deps.url_repo, &deps.url_info, message))
        .collect();
    let results = futures::future::join_all(tasks).await; // Run tasks concurrently

    let failure_ids: Vec<String> = results
        .into_iter()
        .zip(payload.records.into_iter())
        .filter_map(|(result, message)| {
            if let Err(e) = result {
                tracing::error!("Failed to process message {:?}: {}", message.message_id, e);
                Some(message.message_id.unwrap_or_default())
            } else {
                None
            }
        })
        .collect();

    link_created_counter.add(batch_count as u64 - failure_ids.len() as u64, &[]);

    sqs_batch_response.set_failures(failure_ids);
    Ok(sqs_batch_response)
}

#[tracing::instrument("process link_created.v1", skip(url_repo, url_info, message), fields(
    messaging.message.id = tracing::field::Empty,
    messaging.operation.name = "process",
    messaging.destination = "aws_sqs",
    messaging.client.id = "process_link_created",
))]
async fn process_message<R: UrlRepository, I: UrlInfo>(
    url_repo: &R,
    url_info: &I,
    message: SqsMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if message.body.clone().is_none() {
        tracing::warn!(
            "Discarding empty SQS message body for message {:?}",
            message.message_id
        );
        // NOTE: we don't add this to the failed list as we don't want to reprocess it
        return Ok(());
    }

    let current_span = tracing::Span::current();
    let cloud_event: cloudevents::Event =
        match serde_json::from_str(message.body.as_ref().unwrap_or(&"".to_string())) {
            Ok(event) => event,
            Err(e) => {
                tracing::error!("Failed to deserialize CloudEvent: {:?}", e);
                return Err(Box::new(e));
            }
        };

    tracing::Span::current().record("messaging.message.id", &cloud_event.id().to_string());

    add_span_link_from(&current_span, &cloud_event);

    let cloud_event_data = cloud_event.data().ok_or("CloudEvent has no data")?;

    let short_url: ShortUrl = match cloud_event_data {
        cloudevents::Data::Binary(items) => serde_json::from_slice(items)?,
        cloudevents::Data::String(string_data) => serde_json::from_str(&string_data)?,
        cloudevents::Data::Json(value) => serde_json::from_value(value.clone())?,
    };

    let info = url_info.fetch_details(&short_url.original_link).await?;
    tracing::debug!(
        "Fetched info for URL {}: {:?}",
        short_url.original_link,
        info
    );
    url_repo
        .add_details_to_short_url(short_url.link_id, info)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{function_handler, HandlerDeps};
    use aws_lambda_events::{event::sqs::SqsEvent, sqs::SqsMessage};
    use lambda_runtime::{Context, LambdaEvent};
    use mockall::predicate::eq;
    use serde_json::json;
    use shared::{
        core::{MockUrlInfo, MockUrlRepository},
        url_info::UrlDetails,
    };

    fn create_sqs_message(message_id: &str, body: Option<String>) -> SqsMessage {
        let mut message = SqsMessage::default();
        message.message_id = Some(message_id.to_string());
        message.body = body;
        message
    }

    fn create_lambda_event(messages: Vec<SqsMessage>) -> LambdaEvent<SqsEvent> {
        let mut sqs_event = SqsEvent::default();
        sqs_event.records = messages;
        LambdaEvent::new(sqs_event, Context::default())
    }

    #[tokio::test]
    async fn when_valid_message_should_fetch_details_and_update_repository() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_url_info = MockUrlInfo::default();

        mock_url_info
            .expect_fetch_details()
            .times(1)
            .with(eq("https://example.com"))
            .returning(|_| {
                Ok(UrlDetails {
                    title: Some("Example Title".to_string()),
                    description: Some("Example Description".to_string()),
                    content_type: Some("text/html".to_string()),
                })
            });

        mock_url_repo
            .expect_add_details_to_short_url()
            .times(1)
            .with(
                eq("abc123".to_string()),
                mockall::predicate::always(), // UrlDetails doesn't implement PartialEq
            )
            .returning(|_, _| Ok(()));

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            url_info: mock_url_info,
        };

        let body = json!({
            "link_id": "abc123",
            "original_link": "https://example.com",
            "clicks": 0
        })
        .to_string();

        let event = create_lambda_event(vec![create_sqs_message("msg-1", Some(body))]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.batch_item_failures.is_empty());
    }

    #[tokio::test]
    async fn when_message_body_is_empty_should_skip_without_failure() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_url_info = MockUrlInfo::default();

        mock_url_info.expect_fetch_details().times(0);
        mock_url_repo.expect_add_details_to_short_url().times(0);

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            url_info: mock_url_info,
        };

        let event = create_lambda_event(vec![create_sqs_message("msg-1", None)]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.batch_item_failures.is_empty());
    }

    #[tokio::test]
    async fn when_message_body_is_invalid_json_should_report_failure() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_url_info = MockUrlInfo::default();

        mock_url_info.expect_fetch_details().times(0);
        mock_url_repo.expect_add_details_to_short_url().times(0);

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            url_info: mock_url_info,
        };

        let event = create_lambda_event(vec![create_sqs_message(
            "msg-1",
            Some("invalid json".to_string()),
        )]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.batch_item_failures.len(), 1);
        assert_eq!(response.batch_item_failures[0].item_identifier, "msg-1");
    }

    #[tokio::test]
    async fn when_fetch_details_fails_should_report_failure() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_url_info = MockUrlInfo::default();

        mock_url_info
            .expect_fetch_details()
            .times(1)
            .returning(|_| Err("Failed to fetch URL".to_string()));

        mock_url_repo.expect_add_details_to_short_url().times(0);

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            url_info: mock_url_info,
        };

        let body = json!({
            "link_id": "abc123",
            "original_link": "https://example.com",
            "clicks": 0
        })
        .to_string();

        let event = create_lambda_event(vec![create_sqs_message("msg-1", Some(body))]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.batch_item_failures.len(), 1);
        assert_eq!(response.batch_item_failures[0].item_identifier, "msg-1");
    }

    #[tokio::test]
    async fn when_repository_update_fails_should_report_failure() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_url_info = MockUrlInfo::default();

        mock_url_info
            .expect_fetch_details()
            .times(1)
            .returning(|_| Ok(UrlDetails::default()));

        mock_url_repo
            .expect_add_details_to_short_url()
            .times(1)
            .returning(|_, _| Err("DB error".to_string()));

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            url_info: mock_url_info,
        };

        let body = json!({
            "link_id": "abc123",
            "original_link": "https://example.com",
            "clicks": 0
        })
        .to_string();

        let event = create_lambda_event(vec![create_sqs_message("msg-1", Some(body))]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.batch_item_failures.len(), 1);
        assert_eq!(response.batch_item_failures[0].item_identifier, "msg-1");
    }

    #[tokio::test]
    async fn when_multiple_messages_with_mixed_results_should_report_only_failures() {
        let mut mock_url_repo = MockUrlRepository::default();
        let mut mock_url_info = MockUrlInfo::default();

        // First message will succeed, second will fail on fetch_details
        mock_url_info
            .expect_fetch_details()
            .times(1)
            .with(eq("https://success.com"))
            .returning(|_| Ok(UrlDetails::default()));

        mock_url_info
            .expect_fetch_details()
            .times(1)
            .with(eq("https://fail.com"))
            .returning(|_| Err("Network error".to_string()));

        mock_url_repo
            .expect_add_details_to_short_url()
            .times(1)
            .returning(|_, _| Ok(()));

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
            url_info: mock_url_info,
        };

        let body1 = json!({
            "link_id": "success123",
            "original_link": "https://success.com",
            "clicks": 0
        })
        .to_string();

        let body2 = json!({
            "link_id": "fail123",
            "original_link": "https://fail.com",
            "clicks": 0
        })
        .to_string();

        let event = create_lambda_event(vec![
            create_sqs_message("msg-success", Some(body1)),
            create_sqs_message("msg-fail", Some(body2)),
        ]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.batch_item_failures.len(), 1);
        assert_eq!(response.batch_item_failures[0].item_identifier, "msg-fail");
    }
}
