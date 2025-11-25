use aws_config::BehaviorVersion;
use std::env;

#[tokio::main]
async fn main() {
    let queue_url = env::var("QUEUE_URL").expect("QUEUE_URL is not set");

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let sqs_client = aws_sdk_sqs::Client::new(&config);

    let message_body = "Hello from SQS!";

    let result = sqs_client
        .send_message()
        .queue_url(&queue_url)
        .message_body(message_body)
        .send()
        .await;

    match result {
        Ok(output) => {
            println!("Message sent successfully!");
            if let Some(message_id) = output.message_id() {
                println!("Message ID: {}", message_id);
            }
        }
        Err(e) => eprintln!("Error sending message: {:?}", e),
    }
}
