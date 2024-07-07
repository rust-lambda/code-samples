use aws_sdk_dynamodb::types::AttributeValue;
use std::env;

#[tokio::main]
async fn main() {
    let table_name = env::var("TABLE_NAME").expect("TABLE_NAME is not set");
    let last_evaluated_key = env::args().nth(1);
    let config = aws_config::load_from_env().await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let mut scan = dynamodb_client.scan().table_name(&table_name).limit(4);
    if let Some(last_evaluated_key) = last_evaluated_key {
        scan = scan.exclusive_start_key("LinkId", AttributeValue::S(last_evaluated_key));
    }
    let result = scan.send().await;

    match result {
        Ok(output) => {
            if let Some(items) = output.items {
                items.iter().for_each(|item| {
                    println!("{:?}", item);
                });
            }
        }
        Err(e) => eprintln!("Error executing scan: {:?}", e),
    }
}
