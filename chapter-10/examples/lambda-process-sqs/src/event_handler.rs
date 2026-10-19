use ::tracing::Span;
use aws_lambda_events::{event::sqs::SqsEvent, sqs::SqsMessage};
use lambda_runtime::{tracing, Error, LambdaEvent};
use serde::Deserialize;
use shared::observability::add_span_link_from;

#[derive(Deserialize)]
struct MyMessage {
    task_id: String,
    data: String,
}

#[tracing::instrument(skip(event))]
pub async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<(), Error> {
    for record in event.payload.records {
        process_message(record).await?;
    }

    Ok(())
}

#[tracing::instrument(skip(message))]
async fn process_message(message: SqsMessage) -> Result<(), Error> {
    // Get the message body
    let current_span = Span::current();
    tracing::info!("Received message: {:?}", message.body);

    let cloud_event: cloudevents::Event =
        match serde_json::from_str(message.body.as_ref().unwrap_or(&"".to_string())) {
            Ok(event) => event,
            Err(e) => {
                tracing::error!("Failed to deserialize CloudEvent: {:?}", e);
                return Err(Error::from(e));
            }
        };

    add_span_link_from(&current_span, &cloud_event);

    // Parse the JSON message
    let cloud_event_data = cloud_event.data().ok_or("CloudEvent has no data")?;

    let message: MyMessage = match cloud_event_data {
        cloudevents::Data::Binary(items) => serde_json::from_slice(items)?,
        cloudevents::Data::String(string_data) => serde_json::from_str(&string_data)?,
        cloudevents::Data::Json(value) => serde_json::from_value(value.clone())?,
    };

    // Process the message
    println!("Processing task: {} - {}", message.task_id, message.data);

    // Your business logic here
    println!("Task {} completed", message.task_id);
    Ok(())
}
