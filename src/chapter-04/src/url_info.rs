use reqwest::Client;
use scraper::{selector::Selector, Html};

#[derive(Debug)]
pub struct UrlInfo {
    pub http_client: Client,
}

#[derive(Debug, Default)]
pub struct UrlDetails {
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
}

impl UrlInfo {
    pub fn new(http_client: Client) -> Self {
        Self { http_client }
    }

    pub async fn fetch_details(&self, url: &str) -> Result<UrlDetails, String> {
        let response = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Cannot scrape '{}': {}", url, e))?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.chars().take(32).collect::<String>());

        let mut title = None;
        let mut description = None;
        if matches!(content_type, Some(ref t) if t.starts_with("text/html")) {
            // ...
            let html_body = response
                .text()
                .await
                .map_err(|e| format!("Cannot read response body: {}", e))?;

            let document = Html::parse_document(&html_body);
            if let Some(title_element) = document.select(&Selector::parse("title").unwrap()).next()
            {
                title = Some(
                    title_element
                        .inner_html()
                        .chars()
                        .take(256)
                        .collect::<String>(),
                );
            }
            if let Some(description_element) = document
                .select(&Selector::parse("meta[name=description]").unwrap())
                .next()
            {
                description = description_element
                    .value()
                    .attr("content")
                    .map(|s| s.chars().take(256).collect::<String>());
            }
        }

        Ok(UrlDetails {
            content_type,
            title,
            description,
        })
    }
}
