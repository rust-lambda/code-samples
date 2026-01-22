use crate::{
    configuration::Configuration,
    core::{ShortUrl, UrlRepository},
};
use async_trait::async_trait;
use aws_sdk_dynamodb::{
    types::{AttributeValue, ReturnValue},
    Client,
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct DynamoDbUrlRepository {
    dynamodb_client: Client,
}

impl DynamoDbUrlRepository {
    pub fn new(dynamodb_client: Client) -> Self {
        Self { dynamodb_client }
    }
}

#[async_trait]
impl<'a> UrlRepository for DynamoDbUrlRepository {
    async fn get_url_from_short_link(
        &self,
        configuration: &Configuration,
        short_link: &str,
    ) -> Result<Option<String>, String> {
        let result = self
            .dynamodb_client
            .update_item()
            .table_name(&configuration.table_name)
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
                if e.into_service_error()
                    .is_conditional_check_failed_exception()
                {
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
        configuration: &Configuration,
        url_to_shorten: String,
        short_url: String,
        url_details: crate::url_info::UrlDetails,
    ) -> Result<ShortUrl, String> {
        let mut put_item = self
            .dynamodb_client
            .put_item()
            .table_name(&configuration.table_name)
            .item("LinkId", AttributeValue::S(short_url.clone()))
            .item("OriginalLink", AttributeValue::S(url_to_shorten.clone()))
            .item("Clicks", AttributeValue::N("0".to_string()));

        if let Some(ref title) = url_details.title {
            put_item = put_item.item("Title", AttributeValue::S(title.to_string()));
        }
        if let Some(ref description) = url_details.description {
            put_item = put_item.item("Description", AttributeValue::S(description.to_string()));
        }
        if let Some(ref content_type) = url_details.content_type {
            put_item = put_item.item("ContentType", AttributeValue::S(content_type.to_string()));
        }

        put_item
            .condition_expression("attribute_not_exists(LinkId)")
            .send()
            .await
            .map(|_| {
                ShortUrl::new(
                    short_url,
                    url_to_shorten,
                    0,
                    url_details.title,
                    url_details.description,
                    url_details.content_type,
                )
            })
            .map_err(|e| format!("Error adding item: {:?}", e))
    }

    async fn list_urls(
        &self,
        configuration: &Configuration,
        last_evaluated_id: Option<String>,
    ) -> Result<(Vec<ShortUrl>, Option<String>), String> {
        let mut scan = self
            .dynamodb_client
            .scan()
            .table_name(&configuration.table_name)
            .limit(50);
        if let Some(last_evaluated_id) = last_evaluated_id {
            scan = scan
                .exclusive_start_key("LinkId", AttributeValue::S(last_evaluated_id.to_string()));
        }
        let result = scan
            .send()
            .await
            .map_err(|e| format!("Error executing scan: {:?}", e))?;

        let short_urls: Vec<ShortUrl> = result
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| ShortUrl::try_from(item).ok())
            .collect();

        let last_evaluated_id = result
            .last_evaluated_key
            .unwrap_or_default()
            .get("LinkId")
            .and_then(|s| s.as_s().ok().map(|v| v.to_string()));

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

        Ok(ShortUrl::new(
            link_id,
            original_link,
            clicks,
            title,
            description,
            content_type,
        ))
    }
}
