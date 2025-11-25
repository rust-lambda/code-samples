use aws_config::BehaviorVersion;
use serde::Serialize;
use std::env;

#[derive(Serialize)]
struct ScrapeLinkMessage {
    link_id: String,
    target_url: String,
}

#[tokio::main]
async fn main() {
    let queue_url = env::var("QUEUE_URL").expect("QUEUE_URL is not set");

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let sqs_client = aws_sdk_sqs::Client::new(&config);

    let message = ScrapeLinkMessage {
        link_id: "abc123".to_string(),
        target_url: "https://example.com".to_string(),
    };

    let message_body = serde_json::to_string(&message).expect("Failed to serialize message");

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
