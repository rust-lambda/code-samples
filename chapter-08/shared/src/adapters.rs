use crate::{
    core::{ShortUrl, UrlRepository},
    url_info::UrlDetails,
};
use async_trait::async_trait;
use aws_sdk_dynamodb::{
    types::{AttributeValue, ReturnValue},
    Client,
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct DynamoDbUrlRepository {
    table_name: String,
    dynamodb_client: Client,
}

impl DynamoDbUrlRepository {
    pub fn new(table_name: String, dynamodb_client: Client) -> Self {
        Self {
            table_name,
            dynamodb_client,
        }
    }
}

#[async_trait]
impl UrlRepository for DynamoDbUrlRepository {
    async fn get_url_from_short_link(&self, short_link: &str) -> Result<Option<String>, String> {
        let result = self
            .dynamodb_client
            .update_item()
            .table_name(&self.table_name)
            .key("LinkId", AttributeValue::S(short_link.to_string()))
            .update_expression("SET Clicks = Clicks + :val")
            .expression_attribute_values(":val", AttributeValue::N("1".to_string()))
            .condition_expression("attribute_exists(LinkId)")
            .return_values(ReturnValue::AllNew)
            .send()
            .await
            .map(|record| {
                record.attributes.and_then(|attributes| {
                    attributes
                        .get("OriginalLink")
                        .and_then(|v| v.as_s().cloned().ok())
                })
            });

        match result {
            Err(e) => {
                let generic_err_msg = format!("Error incrementing clicks: {:?}", e);
                let service_error = e.into_service_error();
                if service_error.is_conditional_check_failed_exception() {
                    Ok(None)
                } else {
                    Err(generic_err_msg)
                }
            }
            Ok(result) => Ok(result),
        }
    }

    async fn store_short_url(
        &self,
        url_to_shorten: String,
        short_url: String,
    ) -> Result<ShortUrl, String> {
        let put_item = self
            .dynamodb_client
            .put_item()
            .table_name(&self.table_name)
            .item("LinkId", AttributeValue::S(short_url.clone()))
            .item("OriginalLink", AttributeValue::S(url_to_shorten.clone()))
            .item("Clicks", AttributeValue::N("0".to_string()));

        put_item
            .condition_expression("attribute_not_exists(LinkId)")
            .send()
            .await
            .map(|_| ShortUrl::new(short_url, url_to_shorten))
            .map_err(|e| format!("Error adding item: {:?}", e))
    }

    async fn add_details_to_short_url(
        &self,
        short_link: String,
        url_details: UrlDetails,
    ) -> Result<(), String> {
        let mut update_item = self
            .dynamodb_client
            .update_item()
            .table_name(&self.table_name)
            .key("LinkId", AttributeValue::S(short_link.to_string()));

        if let Some(title) = url_details.title {
            update_item = update_item
                .update_expression("SET Title = :title")
                .expression_attribute_values(":title", AttributeValue::S(title));
        }
        if let Some(description) = url_details.description {
            update_item = update_item
                .update_expression("SET Description = :description")
                .expression_attribute_values(":description", AttributeValue::S(description));
        }
        if let Some(content_type) = url_details.content_type {
            update_item = update_item
                .update_expression("SET ContentType = :content_type")
                .expression_attribute_values(":content_type", AttributeValue::S(content_type));
        }

        update_item
            .send()
            .await
            .map(|_| ())
            .map_err(|e| format!("Error updating item: {:?}", e))
    }

    async fn increment_clicks(&self, short_link: &str, n: u32) -> Result<(), String> {
        self.dynamodb_client
            .update_item()
            .table_name(&self.table_name)
            .key("LinkId", AttributeValue::S(short_link.to_string()))
            .update_expression("SET Clicks = Clicks + :val")
            .expression_attribute_values(":val", AttributeValue::N(n.to_string()))
            .send()
            .await
            .map(|_| ())
            .map_err(|e| format!("Error incrementing clicks: {:?}", e))
    }

    async fn list_urls(
        &self,
        last_evaluated_id: Option<String>,
    ) -> Result<(Vec<ShortUrl>, Option<String>), String> {
        let mut scan = self
            .dynamodb_client
            .scan()
            .table_name(&self.table_name)
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

        Ok((short_urls, last_evaluated_id))
    }
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
        let content_type = item
            .get("ContentType")
            .and_then(|c| c.as_s().map(|s| s.to_string()).ok());
        let title = item
            .get("Title")
            .and_then(|c| c.as_s().map(|s| s.to_string()).ok());
        let description = item
            .get("Description")
            .and_then(|c| c.as_s().map(|s| s.to_string()).ok());

        Ok(ShortUrl::with_details(
            link_id,
            original_link,
            clicks,
            title,
            description,
            content_type,
        ))
    }
}
