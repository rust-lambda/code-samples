use aws_lambda_events::{
    event::kinesis::KinesisEvent,
    kinesis::KinesisEventRecord,
    streams::{KinesisBatchItemFailure, KinesisEventResponse},
};
use lambda_runtime::{Error, LambdaEvent};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OrderEvent {
    order_id: String,
    customer_id: String,
    amount: f64,
}

pub async fn function_handler(
    event: LambdaEvent<KinesisEvent>,
) -> Result<KinesisEventResponse, Error> {
    let mut response = KinesisEventResponse::default();

    for record in event.payload.records {
        // Process the order
        match process_record(&record).await {
            Ok(_) => {
                println!("Processed record: {}", record.kinesis.sequence_number);
            }
            Err(e) => {
                eprintln!("Failed to process order: {:?}", e);
                let mut failure = KinesisBatchItemFailure::default();
                failure.item_identifier = Some(record.kinesis.sequence_number);
                response.batch_item_failures.push(failure);
            }
        }
    }

    Ok(response)
}

async fn process_record(record: &KinesisEventRecord) -> Result<(), Error> {
    // Decode and parse
    let data = record.kinesis.data.as_slice();
    let order_event: OrderEvent = serde_json::from_slice(data)?;

    // 50% chance to error
    if rand::random::<f32>() < 0.5 {
        return Err(Error::from("Simulated processing error"));
    }

    // Your business logic here
    println!(
        "Order {} for customer {} with amount ${}",
        order_event.order_id, order_event.customer_id, order_event.amount
    );
    Ok(())
}
