use reqwest::Client;
use serverless_link_shortener_chapter_04::url_info::UrlInfo;

#[tokio::main]
async fn main() {
    let http_client = Client::new();
    let scraper = UrlInfo::new(http_client);

    println!(
        "{:#?}",
        scraper
            .fetch_details("https://loige.co/migrating-from-gatsby-to-astro/")
            .await
            .unwrap()
    );
}
//
