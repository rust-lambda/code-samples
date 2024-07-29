use std::time::Duration;

#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("Failed to create client");

    let html_content = client
        .get("https://www.rust-lang.org")
        .send()
        .await
        .expect("Failed to send request")
        .text()
        .await
        .expect("Failed to get response text");

    println!("{}", html_content);
}
