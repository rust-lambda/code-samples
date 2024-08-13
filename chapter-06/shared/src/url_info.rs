use crate::core::UrlInfo;
use async_trait::async_trait;
use reqwest::Client;
use scraper::{selector::Selector, Html};

#[derive(Debug)]
pub struct HttpUrlInfo {
    pub http_client: Client,
}

#[derive(Debug, Default)]
pub struct UrlDetails {
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
}

impl HttpUrlInfo {
    pub fn new(http_client: Client) -> Self {
        Self { http_client }
    }
}

#[async_trait]
impl UrlInfo for HttpUrlInfo {
    async fn fetch_details(&self, url: &str) -> Result<UrlDetails, String> {
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
        if matches!(content_type, Some(ref ct) if ct.starts_with("text/html")) {
            if let Ok(html_body) = response.text().await {
                let document = Html::parse_document(&html_body);
                if let Some(title_element) = document
                    .select(&Selector::parse("head > title").unwrap())
                    .next()
                {
                    title = Some(
                        title_element
                            .inner_html()
                            .trim()
                            .chars()
                            .take(256)
                            .collect::<String>(),
                    );
                }
                if let Some(description_element) = document
                    .select(&Selector::parse("head > meta[name=description]").unwrap())
                    .next()
                {
                    description = description_element
                        .value()
                        .attr("content")
                        .map(|s| s.chars().take(256).collect::<String>());
                }
            }
        }

        Ok(UrlDetails {
            content_type,
            title,
            description,
        })
    }
}
