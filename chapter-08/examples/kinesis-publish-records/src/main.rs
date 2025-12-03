use aws_config::BehaviorVersion;
use aws_sdk_kinesis::primitives::Blob;
use serde::Serialize;
use std::env;

#[derive(Serialize)]
struct OrderEvent {
    order_id: String,
    customer_id: String,
    amount: f64,
}

#[tokio::main]
async fn main() {
    let stream_name = env::var("STREAM_NAME")
        .expect("STREAM_NAME is not set");

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let kinesis_client = aws_sdk_kinesis::Client::new(&config);

    let event = OrderEvent {
        order_id: "order-123".to_string(),
        customer_id: "customer-456".to_string(),
        amount: 99.99,
    };

    let data = serde_json::to_vec(&event).expect("Failed to serialize");
    let partition_key = event.order_id.clone();

    let result = kinesis_client
        .put_record()
        .stream_name(&stream_name)
        .partition_key(partition_key)
        .data(Blob::new(data))
        .send()
        .await;

    match result {
        Ok(output) => {
            println!("Record published successfully!");
            println!("Shard ID: {}", output.shard_id());
            println!("Sequence Number: {}", output.sequence_number());
        }
        Err(e) => eprintln!("Error publishing record: {:?}", e),
    }
}