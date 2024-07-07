use reqwest::{Client, Url};
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
    pub preview_image_url: Option<String>,
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
        let mut preview_image_url = None;
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

                if let Some(og_image_element) = document
                    .select(&Selector::parse("head > meta[property='og:image']").unwrap())
                    .next()
                {
                    preview_image_url = og_image_element
                        .value()
                        .attr("content")
                        .map(|s| s.chars().take(256).collect::<String>());
                } else if let Some(image_element) = document
                    .select(&Selector::parse("body img").unwrap())
                    .next()
                {
                    // An image source can be a relative URL, we want the absolute version, so we have to normalise it
                    let base_url = Url::parse(url).unwrap();
                    if let Some(image_path) = image_element.value().attr("src") {
                        if let Ok(image_url) = base_url.join(image_path) {
                            preview_image_url = Some(image_url.to_string());
                        }
                    }
                }
            }
        }

        Ok(UrlDetails {
            content_type,
            title,
            description,
            preview_image_url,
        })
    }
}
