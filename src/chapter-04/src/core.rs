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
    pub fn new(dynamodb_urls_table: String, dynamodb_client: Client) -> Self {
        Self {
            dynamodb_urls_table,
            dynamodb_client,
        }
    }

    pub async fn shorten_url(&self, req: ShortenUrlRequest) -> Result<ShortenUrlResponse, String> {
        let short_url = self.generate_short_url();

        let result = self
            .dynamodb_client
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
            .await;

        match result {
            Ok(_) => Ok(ShortenUrlResponse {
                shortened_url: short_url,
            }),
            Err(e) => return Err(format!("Failed to shorten URL: {:?}", e)),
        }
    }

    pub async fn retrieve_url(&self, short_url: String) -> Result<Option<String>, String> {
        let result = self
            .dynamodb_client
            .get_item()
            .table_name(&self.dynamodb_urls_table)
            .key("LinkId", AttributeValue::S(short_url.clone()))
            .send()
            .await;

        result
            .map_err(|e| format!("Error getting item: {:?}", e))
            .map(|record| {
                record.item.and_then(|attributes| {
                    attributes
                        .get("OriginalLink")
                        .map(|v| v.as_s().unwrap().clone())
                })
            })
    }

    fn generate_short_url(&self) -> String {
        let idgen = CuidConstructor::new().with_length(10);
        idgen.create_id()
    }
}
