use std::collections::HashMap;

use aws_sdk_dynamodb::{types::AttributeValue, Client};
use cuid2::CuidConstructor;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ShortenUrlRequest {
    url_to_shorten: String,
}

#[derive(Serialize)]
pub struct ShortenUrlResponse {
    shortened_url: String,
}

#[derive(Debug)]
pub struct UrlShortener {
    dynamodb_urls_table: String,
    dynamodb_client: Client,
}

#[derive(Debug, Serialize)]
pub struct ShortUrl {
    link_id: String,
    original_link: String,
    clicks: u32,
}

impl TryFrom<HashMap<String, AttributeValue>> for ShortUrl {
    type Error = String;

    fn try_from(item: HashMap<String, AttributeValue>) -> Result<Self, Self::Error> {
        let link_id = item
            .get("LinkId")
            .ok_or_else(|| "LinkId not found".to_string())?
            .as_s()
            .map(|s| s.to_string())
            .map_err(|_| "LinkId is not a String".to_string())?;
        let original_link = item
            .get("OriginalLink")
            .ok_or_else(|| "OriginalLink not found".to_string())?
            .as_s()
            .map(|s| s.to_string())
            .map_err(|_| "OriginalLink is not a String".to_string())?;
        let clicks = item
            .get("Clicks")
            .ok_or_else(|| "Clicks not found".to_string())?
            .as_n()
            .map_err(|_| "Clicks is not a number".to_string())
            .and_then(|n| {
                n.parse::<u32>()
                    .map_err(|_| "Cannot convert Clicks into u32".to_string())
            })?;

        Ok(Self {
            link_id,
            original_link,
            clicks,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ListShortUrlsResponse {
    short_urls: Vec<ShortUrl>,
    last_evaluated_id: Option<String>,
}

impl UrlShortener {
    pub fn new(dynamodb_urls_table: &str, dynamodb_client: Client) -> Self {
        Self {
            dynamodb_urls_table: dynamodb_urls_table.to_string(),
            dynamodb_client,
        }
    }

    pub async fn shorten_url(&self, req: ShortenUrlRequest) -> Result<ShortenUrlResponse, String> {
        let short_url = self.generate_short_url();

        self.dynamodb_client
            .put_item()
            .table_name(&self.dynamodb_urls_table)
            .item("LinkId", AttributeValue::S(short_url.clone()))
            .item(
                "OriginalLink",
                AttributeValue::S(req.url_to_shorten.clone()),
            )
            .item("Clicks", AttributeValue::N("0".to_string()))
            .condition_expression("attribute_not_exists(LinkId)")
            .send()
            .await
            .map(|_| ShortenUrlResponse {
                shortened_url: short_url,
            })
            .map_err(|e| format!("Error adding item: {:?}", e))
    }

    pub async fn retrieve_url(&self, short_url: &str) -> Result<Option<String>, String> {
        self.dynamodb_client
            .get_item()
            .table_name(&self.dynamodb_urls_table)
            .key("LinkId", AttributeValue::S(short_url.to_string()))
            .send()
            .await
            .map_err(|e| format!("Error getting item: {:?}", e))
            .map(|record| {
                record.item.and_then(|attributes| {
                    attributes
                        .get("OriginalLink")
                        .and_then(|v| v.as_s().cloned().ok())
                })
            })
    }

    pub async fn increment_clicks(&self, link_id: &str) -> Result<(), String> {
        self.dynamodb_client
            .update_item()
            .table_name(&self.dynamodb_urls_table)
            .key("LinkId", AttributeValue::S(link_id.to_string()))
            .update_expression("SET Clicks = Clicks + :val")
            .expression_attribute_values(":val", AttributeValue::N("1".to_string()))
            .send()
            .await
            .map(|_| ())
            .map_err(|e| format!("Error incrementing clicks: {:?}", e))
    }

    pub async fn list_urls(
        &self,
        last_evaluated_id: Option<&str>,
    ) -> Result<ListShortUrlsResponse, String> {
        let mut scan = self
            .dynamodb_client
            .scan()
            .table_name(&self.dynamodb_urls_table)
            .limit(50);
        if let Some(last_evaluated_id) = last_evaluated_id {
            scan = scan
                .exclusive_start_key("LinkId", AttributeValue::S(last_evaluated_id.to_string()));
        }
        let result = scan
            .send()
            .await
            .map_err(|e| format!("Error executing scan: {:?}", e))?;

        let mut short_urls = vec![];
        if let Some(items) = result.items {
            for item in items {
                // ignore item that cannot be properly deserialized
                if let Ok(short_url) = ShortUrl::try_from(item) {
                    short_urls.push(short_url);
                }
            }
        }
        let last_evaluated_id = result
            .last_evaluated_key
            .unwrap_or_default()
            .get("LinkId")
            .map(|s| s.as_s().unwrap().to_string());

        Ok(ListShortUrlsResponse {
            short_urls,
            last_evaluated_id,
        })
    }

    fn generate_short_url(&self) -> String {
        let idgen = CuidConstructor::new().with_length(10);
        idgen.create_id()
    }
}
