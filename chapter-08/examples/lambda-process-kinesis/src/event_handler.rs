use aws_lambda_events::event::kinesis::KinesisEvent;
use lambda_runtime::{Error, LambdaEvent};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OrderEvent {
    order_id: String,
    customer_id: String,
    amount: f64,
}

pub async fn function_handler(event: LambdaEvent<KinesisEvent>) -> Result<(), Error> {
    for record in event.payload.records {
        // Decode base64-encoded data
        let data = record.kinesis.data.as_slice();

        // Parse JSON
        let order_event: OrderEvent = serde_json::from_slice(data)?;

        // Process the order
        println!("Processing order: {}", order_event.order_id);
        println!(
            "Order {} for customer {} with amount ${}",
            order_event.order_id, order_event.customer_id, order_event.amount
        );
    }

    Ok(())
}
