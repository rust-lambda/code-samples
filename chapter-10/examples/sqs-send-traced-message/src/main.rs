use aws_config::BehaviorVersion;
use cloudevents::{AttributesReader, EventBuilder, EventBuilderV10};
use serde::Serialize;
use shared::{
    core::{CuidGenerator, IdGenerator},
    observability::init_otel,
};
use std::env;
use tracing::{info, Span};

#[derive(Serialize)]
struct ScrapeLinkMessage {
    link_id: String,
    target_url: String,
}

#[tokio::main]
async fn main() {
    let _otel_guard = init_otel().expect("Failed to initialize telemetry");

    run_message_sender().await;
}

#[tracing::instrument()]
async fn run_message_sender() {
    let current_span = Span::current();
    let extension_value = shared::observability::get_traceparent_extension_value(&current_span);

    let message = ScrapeLinkMessage {
        link_id: "abc123".to_string(),
        target_url: "https://example.com".to_string(),
    };

    let message_body = serde_json::to_string(&message).expect("Failed to serialize message");

    let event = EventBuilderV10::new()
        .id(CuidGenerator::new().generate_id().to_string())
        .ty("scrape_link_message.v1")
        .source("http://dev.example.com")
        .data("application/json", message_body)
        .extension("traceparent", extension_value)
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build CloudEvent: {}", e);
            e
        })
        .unwrap();

    publish_message(&event).await;
}

#[tracing::instrument("publish scrape_link_message.v1", fields(
    messaging.message.id = tracing::field::Empty,
    messaging.operation.name = "publish",
    messaging.destination = "aws_sqs",
    messaging.client.id = "sqs_publisher",
))]
async fn publish_message(cloud_event: &cloudevents::Event) {
    let queue_url = env::var("QUEUE_URL").expect("QUEUE_URL is not set");

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let sqs_client = aws_sdk_sqs::Client::new(&config);

    tracing::Span::current().record("messaging.message.id", &cloud_event.id().to_string());

    // Here you would publish the message to your messaging system
    tracing::info!(
        "Published message with ID: {}",
        &cloud_event.id().to_string()
    );

    let event_as_json =
        serde_json::to_string(&cloud_event).expect("Failed to serialize CloudEvent");

    tracing::info!("Sending message to SQS queue: {}", &queue_url);
    tracing::info!("Message body is: {}", &event_as_json);

    let result = sqs_client
        .send_message()
        .queue_url(&queue_url)
        .message_body(event_as_json)
        .send()
        .await;

    match result {
        Ok(output) => {
            info!(
                "Message sent successfully. Message ID: {:?}",
                output.message_id()
            );
        }
        Err(e) => tracing::error!("Error sending message: {:?}", e),
    }
}
