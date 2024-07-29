use aws_sdk_dynamodb::types::AttributeValue;
use std::env;

#[tokio::main]
async fn main() {
    let table_name = env::var("TABLE_NAME").expect("TABLE_NAME is not set");

    let config = aws_config::load_from_env().await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let result = dynamodb_client
        .put_item()
        .table_name(&table_name)
        .item("LinkId", AttributeValue::S("y3cfw1hafb".to_string()))
        .item(
            "OriginalLink",
            AttributeValue::S(
                "https://www.example.com/very-long-url-that-we-want-to-shorten".to_string(),
            ),
        )
        .item("Clicks", AttributeValue::N("0".to_string()))
        .send()
        .await;

    match result {
        Ok(_) => println!("Item added successfully"),
        Err(e) => eprintln!("Error adding item: {:?}", e),
    }
}
