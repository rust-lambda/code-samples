use aws_sdk_cloudformation::types::Output;
use reqwest::redirect::Policy;
use reqwest::Client;
use shared::core::ShortUrl;
use std::env;

#[ignore]
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
        serde_json::from_str(response.text().await.unwrap().as_str()).unwrap();

    assert_eq!(response_data.original_link, "https://google.com");

    let redirect_response = http_client
        .get(format!("{}{}", api_endpoint, response_data.link_id))
        .send()
        .await
        .unwrap();

    assert_eq!(redirect_response.status(), 302);
}

async fn retrieve_api_endpoint() -> String {
    let config = aws_config::load_from_env().await;
    let cloudformation_client = aws_sdk_cloudformation::Client::new(&config);
    let stack_name = env::var("STACK_NAME").unwrap_or("rust-link-shorten".to_string());

    let get_stacks = cloudformation_client
        .describe_stacks()
        .set_stack_name(Some(stack_name))
        .send()
        .await
        .unwrap();

    let outputs = get_stacks.stacks.unwrap()[0].clone().outputs.unwrap();
    let api_outputs: Vec<Output> = outputs
        .into_iter()
        .filter(|output| output.output_key.clone().unwrap() == "UrlShortenerEndpoint")
        .collect();

    api_outputs[0].clone().output_value.unwrap()
}
