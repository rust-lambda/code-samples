use aws_lambda_events::{
    event::sqs::SqsEvent,
    sqs::{SqsBatchResponse, SqsMessage},
};
use lambda_runtime::{Error, LambdaEvent};

pub async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<SqsBatchResponse, Error> {
    let mut sqs_batch_response = SqsBatchResponse::default();

    for record in event.payload.records {
        let message_id = record.message_id.clone().unwrap_or_default();

        // Try to process the message
        if let Err(e) = process_record(&record).await {
            println!("Failed to process message {}: {}", message_id, e);
            // Add to failures list so it will be retried
            sqs_batch_response.add_failure(message_id);
        }
    }

    // Return the batch response with any failed messages
    Ok(sqs_batch_response)
}

async fn process_record(record: &SqsMessage) -> Result<(), Error> {
    // 50% chance to error
    if rand::random::<f32>() < 0.5 {
        return Err(Error::from("Simulated processing error"));
    }

    // Your business logic here
    println!("Record {:?} processed", record);

    Ok(())
}
