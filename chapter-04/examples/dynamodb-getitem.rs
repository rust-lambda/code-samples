use aws_sdk_dynamodb::types::AttributeValue;
use std::env;

#[tokio::main]
async fn main() {
    let table_name = env::var("TABLE_NAME").expect("TABLE_NAME is not set");

    let config = aws_config::load_from_env().await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let result = dynamodb_client
        .get_item()
        .table_name(&table_name)
        .key("LinkId", AttributeValue::S("y3cfw1hafb".to_string()))
        .send()
        .await;

    match result {
        Ok(record) => match record.item {
            Some(item) => println!("Item retrieved successfully: {:?}", item),
            None => println!("Item not found"),
        },
        Err(e) => eprintln!("Error getting item: {:?}", e),
    }
}
