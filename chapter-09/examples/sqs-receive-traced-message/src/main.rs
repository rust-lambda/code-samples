use aws_config::BehaviorVersion;
use aws_sdk_sqs::types::Message;
use cloudevents::AttributesReader;
use shared::observability::{add_span_link_from, init_otel};
use std::{env, time::Duration};
use tracing::Span;

#[tokio::main]
async fn main() {
    let _otel_guard = init_otel().expect("Failed to initialize telemetry");

    receive_sqs_message().await;
}

#[tracing::instrument("receive scrape_link_message.v1", fields(
    messaging.operation.name = "receive",
    messaging.destination = "aws_sqs",
    messaging.client.id = "sqs_receiver",
    messaging.batch.message_count = tracing::field::Empty,
    messaging.consumer.group.name = "sqs_receiver",
))]
async fn receive_sqs_message() {
    let queue_url = env::var("QUEUE_URL").expect("QUEUE_URL is not set");

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let sqs_client = aws_sdk_sqs::Client::new(&config);

    let receive = sqs_client
        .receive_message()
        .queue_url(&queue_url)
        .max_number_of_messages(1)
        .send()
        .await;

    match receive {
        Ok(output) => {
            tracing::info!(
                "Received {} messages",
                output.messages.as_ref().map_or(0, |m| m.len())
            );
            tracing::Span::current().record(
                "messaging.batch.message_count",
                output.messages.as_ref().map_or(0, |m| m.len()),
            );
            for message in output.messages.unwrap_or_default().iter() {
                let _ = process_sqs_message(&sqs_client, message).await;
            }
        }
        Err(e) => {
            tracing::error!("Error receiving message: {:?}", e);
            return;
        }
    }
}

#[tracing::instrument("process scrape_link_message.v1", skip(sqs_client, message), fields(
    messaging.message.id = tracing::field::Empty,
    messaging.operation.name = "process",
    messaging.destination = "aws_sqs",
    messaging.client.id = "sqs_receiver",
))]
async fn process_sqs_message(sqs_client: &aws_sdk_sqs::Client, message: &Message) {
    let current_span = Span::current();
    tracing::info!("Received message: {:?}", message.body);

    let cloud_event: cloudevents::Event =
        match serde_json::from_str(message.body.as_ref().unwrap_or(&"".to_string())) {
            Ok(event) => event,
            Err(e) => {
                tracing::error!("Failed to deserialize CloudEvent: {:?}", e);
                return;
            }
        };

    tracing::Span::current().record("messaging.message.id", cloud_event.id().to_string());

    add_span_link_from(&current_span, &cloud_event);

    // Further processing of the message would go here
    tokio::time::sleep(Duration::from_secs(2)).await;

    sqs_client
        .delete_message()
        .queue_url(env::var("QUEUE_URL").expect("QUEUE_URL is not set"))
        .receipt_handle(message.receipt_handle.as_ref().unwrap())
        .send()
        .await
        .expect("Failed to delete message");
}
