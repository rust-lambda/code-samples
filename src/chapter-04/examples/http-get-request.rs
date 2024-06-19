#[tokio::main]
async fn main() {
    let html_content = reqwest::get("https://www.rust-lang.org")
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    println!("{}", html_content);
}
