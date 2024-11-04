use aws_sdk_cloudformation::types::Output;
use reqwest::redirect::Policy;
use reqwest::Client;
use shared::core::ShortUrl;
use std::env;

#[tokio::test]
async fn when_valid_link_is_passed_should_retrieve_info_and_store() {
    let api_endpoint = retrieve_api_endpoint().await;

    let http_client = Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .redirect(Policy::none())
        .build()
        .unwrap();

    let result = http_client
        .post(format!("{}links", api_endpoint))
        .header("Content-Type", "application/json")
        .body(serde_json::json!({"url_to_shorten": "https://google.com"}).to_string())
        .send()
        .await;

    assert!(result.is_ok());

    let response = result.unwrap();

    assert_eq!(response.status(), 200);

    let response_data: ShortUrl =
        serde_json::from_str(response.text().await.unwrap().as_str()).expect("Response to be a JSON string that successfully deserializes to a `ShortUrl` struct");

    assert_eq!(response_data.original_link, "https://google.com");

    let redirect_response = http_client
        .get(format!("{}{}", api_endpoint, response_data.link_id))
        .send()
        .await
        .expect("Accessing redirect should be successful.");

    assert_eq!(redirect_response.status(), 302);
}


#[tokio::test]
async fn when_invalid_body_is_passed_application_should_return_400_error() {
    let api_endpoint = retrieve_api_endpoint().await;

    let http_client = Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .redirect(Policy::none())
        .build()
        .unwrap();

    let result = http_client
        .post(format!("{}links", api_endpoint))
        .header("Content-Type", "application/json")
        .body(serde_json::json!({"this_is_not_a_valid_body": "https://google.com"}).to_string())
        .send()
        .await;

    assert!(result.is_ok());

    let response = result.unwrap();

    assert_eq!(response.status(), 400);
}

async fn retrieve_api_endpoint() -> String {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let cloudformation_client = aws_sdk_cloudformation::Client::new(&config);
    let stack_name = env::var("STACK_NAME").unwrap_or("rust-link-shorten".to_string());
    let env = env::var("ENV").expect("The current environment should be set using the 'ENV' environment variable");

    let get_stacks = cloudformation_client
        .describe_stacks()
        .set_stack_name(Some(stack_name.clone()))
        .send()
        .await
        .expect(format!("CloudFormation stack named {} should exist", stack_name).as_str());

    let outputs = get_stacks.stacks.expect("Get stack request should return an array")[0].clone().outputs.expect("The first stack in the get stacks response should have outputs");
    let api_outputs: Vec<Output> = outputs
        .into_iter()
        .filter(|output| output.export_name.clone().unwrap() == format!("UrlShortenerEndpoint-{}", env))
        .collect();

    api_outputs[0].clone().output_value.expect("CloudFormation stack should have an output named `UrlShortenerEndpoint`")
}
