use aws_lambda_events::event::sqs::SqsEvent;
use lambda_runtime::{Error, LambdaEvent};
use serde::Deserialize;

#[derive(Deserialize)]
struct MyMessage {
    task_id: String,
    data: String,
}

pub async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<(), Error> {
    for record in event.payload.records {
        // Get the message body
        let body = record.body.unwrap_or_default();

        // Parse the JSON message
        let message: MyMessage = serde_json::from_str(&body)?;

        // Process the message
        println!("Processing task: {} - {}", message.task_id, message.data);
        process_task(message).await?;
    }

    Ok(())
}

async fn process_task(message: MyMessage) -> Result<(), Error> {
    // Your business logic here
    println!("Task {} completed", message.task_id);
    Ok(())
}
