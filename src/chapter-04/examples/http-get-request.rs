#[tokio::main]
async fn main() {
    let html_content = reqwest::get("https://www.rust-lang.org")
        .await
        .expect("Failed to send request")
        .text()
        .await
        .expect("Failed to get response text");

    println!("{}", html_content);
}
