use aws_sdk_eventbridge::{operation::put_events::PutEventsError, types::PutEventsRequestEntry};
use aws_sdk_sqs::operation::send_message::SendMessageError;
use shared::core::ShortUrl;
use std::fmt::Display;
use thiserror::Error;

#[cfg(test)]
use mockall::automock;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[cfg_attr(test, automock)]
pub(crate) trait EventPublisher {
    async fn publish_link_created(&self, short_url: &ShortUrl) -> Result<(), Error>;
}

#[derive(Debug, Error)]
struct SqsEventBridgePublisherError {
    sqs_error: Option<SendMessageError>,
    eventbridge_error: Option<PutEventsError>,
}

impl Display for SqsEventBridgePublisherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SqsEventBridgePublisherError")?;
        if let Some(sqs_err) = &self.sqs_error {
            write!(f, " SQS Error: {}", sqs_err)?;
        }
        if let Some(eb_err) = &self.eventbridge_error {
            write!(f, " EventBridge Error: {}", eb_err)?;
        }
        Ok(())
    }
}

pub(crate) struct SqsEventBridgePublisher {
    pub sqs_client: aws_sdk_sqs::Client,
    pub queue_url: String,
    pub eventbridge_client: aws_sdk_eventbridge::Client,
}

impl SqsEventBridgePublisher {
    pub fn new(
        sqs_client: aws_sdk_sqs::Client,
        queue_url: String,
        eventbridge_client: aws_sdk_eventbridge::Client,
    ) -> Self {
        Self {
            sqs_client,
            queue_url,
            eventbridge_client,
        }
    }
}

impl EventPublisher for SqsEventBridgePublisher {
    async fn publish_link_created(&self, short_url: &ShortUrl) -> Result<(), Error> {
        let message_body = serde_json::to_string(short_url).expect("Failed to serialize ShortUrl");

        let send_to_queue = self
            .sqs_client
            .send_message()
            .queue_url(&self.queue_url)
            .message_body(message_body.clone())
            .send();

        let send_event = self
            .eventbridge_client
            .put_events()
            .entries(
                PutEventsRequestEntry::builder()
                    .source("custom.link_shortener")
                    .detail_type("LinkCreated")
                    .detail(message_body.clone())
                    .build(),
            )
            .send();

        let (sqs_result, eventbridge_result) = tokio::join!(send_to_queue, send_event);
        if sqs_result.is_err() || eventbridge_result.is_err() {
            return Err(Box::new(SqsEventBridgePublisherError {
                sqs_error: sqs_result.err().map(|e| e.into_service_error()),
                eventbridge_error: eventbridge_result.err().map(|e| e.into_service_error()),
            }));
        }

        Ok(())
    }
}
