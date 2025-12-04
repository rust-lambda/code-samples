use aws_lambda_events::{event::kinesis::KinesisEvent, kinesis::KinesisEventRecord};
use cloudevents::AttributesReader;
use lambda_runtime::{tracing, Error, LambdaEvent};
use opentelemetry::global;
use shared::{
    core::{ShortUrl, UrlRepository},
    observability::add_span_link_from,
};
use std::collections::HashMap;

pub(crate) struct HandlerDeps<R: UrlRepository> {
    pub url_repo: R,
}

#[tracing::instrument(skip(deps, event))]
pub(crate) async fn function_handler<R: UrlRepository>(
    deps: &HandlerDeps<R>,
    event: LambdaEvent<KinesisEvent>,
) -> Result<(), Error> {
    let meter = global::meter("process_link_clicked");
    let link_clicked_counter = meter.u64_counter("links_clicked").build();

    // Extract some useful information from the request
    let payload = event.payload;

    // Aggregate clicks by link ID
    let mut clicks_by_id: HashMap<String, u64> = HashMap::new();
    for record in payload.records {
        let process_result = process_message(record).await;

        match process_result {
            Ok(link_id) => {
                let counter = clicks_by_id.entry(link_id).or_insert(0);
                *counter += 1;
            }
            Err(e) => {
                tracing::warn!("Skipping record due to processing error: {:?}", e);
            }
        }
    }

    // Update click counts in the repository (concurrently)
    let mut update_futures = vec![];
    for (link_id, click_count) in clicks_by_id {
        link_clicked_counter.add(
            click_count,
            &[opentelemetry::KeyValue::new("link_id", link_id.clone())],
        );
        
        let repo = &deps.url_repo;
        update_futures.push(async move {
            match repo.increment_clicks(&link_id, click_count).await {
                Err(e) => {
                    tracing::error!(
                        "Failed to update click count for link ID {}: {:?}",
                        link_id,
                        e
                    );
                }
                Ok(_) => {
                    tracing::info!(
                        "Successfully updated click count for link ID {}: +{}",
                        link_id,
                        click_count
                    );
                }
            }
        });
    }
    futures::future::join_all(update_futures).await;

    Ok(())
}

#[tracing::instrument("process link_clicked.v1" skip(record), fields(
    messaging.message.id = tracing::field::Empty,
    messaging.operation.name = "process",
    messaging.destination = "aws_kinesis",
    messaging.client.id = "process_link_clicked",
))]
async fn process_message(
    record: KinesisEventRecord,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let kinesis_record = record.kinesis;
    let data = kinesis_record.data.as_ref();

    let current_span = tracing::Span::current();
    let cloud_event: cloudevents::Event = match serde_json::from_slice(data) {
        Ok(event) => event,
        Err(e) => {
            tracing::error!("Failed to deserialize CloudEvent: {:?}", e);
            return Err(Box::new(e));
        }
    };

    tracing::Span::current().record("messaging.message.id", &cloud_event.id().to_string());

    add_span_link_from(&current_span, &cloud_event);

    let cloud_event_data = cloud_event.data().ok_or("CloudEvent has no data")?;

    let link_click_event: ShortUrl = match cloud_event_data {
        cloudevents::Data::Binary(items) => serde_json::from_slice(items)?,
        cloudevents::Data::String(string_data) => serde_json::from_str(&string_data)?,
        cloudevents::Data::Json(value) => serde_json::from_value(value.clone())?,
    };

    Ok(link_click_event.link_id)
}

#[cfg(test)]
mod tests {
    use super::{function_handler, HandlerDeps};
    use aws_lambda_events::event::kinesis::{KinesisEvent, KinesisEventRecord};
    use lambda_runtime::{Context, LambdaEvent};
    use mockall::predicate::eq;
    use serde_json::json;
    use shared::core::MockUrlRepository;

    fn create_kinesis_record(data: &str) -> KinesisEventRecord {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let encoded_data = STANDARD.encode(data);

        let record_json = json!({
            "kinesis": {
                "data": encoded_data,
                "partitionKey": "test-partition",
                "sequenceNumber": "123",
                "approximateArrivalTimestamp": 1234567890.123
            },
            "eventSource": "aws:kinesis",
            "eventID": "test-event-id",
            "eventName": "aws:kinesis:record",
            "eventSourceARN": "arn:aws:kinesis:us-east-1:123456789:stream/test",
            "awsRegion": "us-east-1"
        });

        serde_json::from_value(record_json).expect("Failed to create KinesisEventRecord")
    }

    fn create_lambda_event(records: Vec<KinesisEventRecord>) -> LambdaEvent<KinesisEvent> {
        let mut kinesis_event = KinesisEvent::default();
        kinesis_event.records = records;
        LambdaEvent::new(kinesis_event, Context::default())
    }

    #[tokio::test]
    async fn when_valid_record_should_increment_clicks() {
        let mut mock_url_repo = MockUrlRepository::default();

        mock_url_repo
            .expect_increment_clicks()
            .times(1)
            .with(eq("abc123"), eq(1u64))
            .returning(|_, _| Ok(()));

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };

        let data = json!({
            "link_id": "abc123",
            "original_link": "https://example.com",
            "clicks": 0
        })
        .to_string();

        let event = create_lambda_event(vec![create_kinesis_record(&data)]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn when_invalid_json_should_skip_and_succeed() {
        let mut mock_url_repo = MockUrlRepository::default();

        mock_url_repo.expect_increment_clicks().times(0);

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };

        let event = create_lambda_event(vec![create_kinesis_record("invalid json")]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn when_multiple_records_same_link_should_aggregate() {
        let mut mock_url_repo = MockUrlRepository::default();

        mock_url_repo
            .expect_increment_clicks()
            .times(1)
            .with(eq("abc123"), eq(3u64))
            .returning(|_, _| Ok(()));

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };

        let data = json!({
            "link_id": "abc123",
            "original_link": "https://example.com",
            "clicks": 0
        })
        .to_string();

        let event = create_lambda_event(vec![
            create_kinesis_record(&data),
            create_kinesis_record(&data),
            create_kinesis_record(&data),
        ]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn when_multiple_different_links_should_update_each() {
        let mut mock_url_repo = MockUrlRepository::default();

        mock_url_repo
            .expect_increment_clicks()
            .times(1)
            .with(eq("link1"), eq(1u64))
            .returning(|_, _| Ok(()));

        mock_url_repo
            .expect_increment_clicks()
            .times(1)
            .with(eq("link2"), eq(1u64))
            .returning(|_, _| Ok(()));

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };

        let data1 = json!({
            "link_id": "link1",
            "original_link": "https://example1.com",
            "clicks": 0
        })
        .to_string();

        let data2 = json!({
            "link_id": "link2",
            "original_link": "https://example2.com",
            "clicks": 0
        })
        .to_string();

        let event = create_lambda_event(vec![
            create_kinesis_record(&data1),
            create_kinesis_record(&data2),
        ]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn when_repository_error_should_log_and_succeed() {
        let mut mock_url_repo = MockUrlRepository::default();

        mock_url_repo
            .expect_increment_clicks()
            .times(1)
            .returning(|_, _| Err("DB error".to_string()));

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };

        let data = json!({
            "link_id": "abc123",
            "original_link": "https://example.com",
            "clicks": 0
        })
        .to_string();

        let event = create_lambda_event(vec![create_kinesis_record(&data)]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn when_empty_records_should_succeed() {
        let mut mock_url_repo = MockUrlRepository::default();

        mock_url_repo.expect_increment_clicks().times(0);

        let deps = HandlerDeps {
            url_repo: mock_url_repo,
        };

        let event = create_lambda_event(vec![]);

        let result = function_handler(&deps, event).await;

        assert!(result.is_ok());
    }
}
