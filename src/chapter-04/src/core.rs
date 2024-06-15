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

    fn generate_short_url(&self) -> String {
        let idgen = CuidConstructor::new().with_length(10);
        idgen.create_id()
    }
}
