use aws_lambda_events::event::eventbridge::EventBridgeEvent;
use lambda_runtime::{Error, LambdaEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderCreatedDetail {
    order_id: String,
    customer_id: String,
    order_value: f64,
}

pub async fn function_handler(
    event: LambdaEvent<EventBridgeEvent<OrderCreatedDetail>>,
) -> Result<(), Error> {
    let detail = &event.payload.detail;

    // TODO: your processing logic here
    println!(
        "Processing order {} for customer {} with value ${}",
        detail.order_id, detail.customer_id, detail.order_value
    );

    println!("Successfully processed order: {}", detail.order_id);

    Ok(())
}
