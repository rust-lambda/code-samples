use aws_config::BehaviorVersion;
use aws_sdk_eventbridge::types::PutEventsRequestEntry;
use serde::Serialize;
use std::env;

#[derive(Serialize)]
struct OrderCreatedDetail {
    order_id: String,
    customer_id: String,
    order_value: f64,
}

#[tokio::main]
async fn main() {
    let event_bus_name = env::var("EVENT_BUS_NAME").unwrap_or("default".to_string());

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = aws_sdk_eventbridge::Client::new(&config);

    let mut entries = Vec::new();

    for i in 0..3 {
        let detail = OrderCreatedDetail {
            order_id: format!("order-{}", i),
            customer_id: format!("customer-{}", i),
            order_value: 100.0 + (i as f64 * 10.0),
        };
        let detail_json = serde_json::to_string(&detail).unwrap();

        let entry = PutEventsRequestEntry::builder()
            .source("custom.myapp")
            .detail_type("OrderCreated")
            .detail(detail_json)
            .event_bus_name(&event_bus_name)
            .build();

        entries.push(entry);
    }

    let response = client.put_events().set_entries(Some(entries)).send().await;

    match response {
        Ok(output) => {
            println!("Request sent successfully!");
            for entry in output.entries() {
                if let Some(event_id) = entry.event_id() {
                    println!("Event ID: {}", event_id);
                } else {
                    println!("Event failed to be recorded: {:?}", entry.error_message());
                }
            }
        }
        Err(e) => eprintln!("Request failed: {:?}", e),
    }
}
