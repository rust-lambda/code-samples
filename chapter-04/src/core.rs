use crate::url_info::UrlInfo;
use aws_sdk_dynamodb::{
    types::{AttributeValue, ReturnValue},
    Client,
};
use cuid2::CuidConstructor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct ShortenUrlRequest {
    url_to_shorten: String,
}

#[derive(Debug)]
pub struct UrlShortener {
    dynamodb_urls_table: String,
    dynamodb_client: Client,
    url_info: UrlInfo,
}

#[derive(Debug, Serialize)]
pub struct ShortUrl {
    link_id: String,
    original_link: String,
    clicks: u32,
    title: Option<String>,
    description: Option<String>,
    content_type: Option<String>,
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

        Ok(Self {
            link_id,
            original_link,
            clicks,
            content_type,
            title,
            description,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ListShortUrlsResponse {
    short_urls: Vec<ShortUrl>,
    last_evaluated_id: Option<String>,
}

impl UrlShortener {
    pub fn new(dynamodb_urls_table: &str, dynamodb_client: Client, url_info: UrlInfo) -> Self {
        Self {
            dynamodb_urls_table: dynamodb_urls_table.to_string(),
            dynamodb_client,
            url_info,
        }
    }

    pub async fn shorten_url(&self, req: ShortenUrlRequest) -> Result<ShortUrl, String> {
        let short_url = self.generate_short_url();
        let url_details = self
            .url_info
            .fetch_details(&req.url_to_shorten)
            .await
            .unwrap_or_default();

        let mut put_item = self
            .dynamodb_client
            .put_item()
            .table_name(&self.dynamodb_urls_table)
            .item("LinkId", AttributeValue::S(short_url.clone()))
            .item(
                "OriginalLink",
                AttributeValue::S(req.url_to_shorten.clone()),
            )
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
            .map(|_| ShortUrl {
                link_id: short_url,
                original_link: req.url_to_shorten.clone(),
                clicks: 0,
                title: url_details.title,
                description: url_details.description,
                content_type: url_details.content_type,
            })
            .map_err(|e| format!("Error adding item: {:?}", e))
    }

    // pub async fn retrieve_url(&self, short_url: &str) -> Result<Option<String>, String> {
    //     self.dynamodb_client
    //         .get_item()
    //         .table_name(&self.dynamodb_urls_table)
    //         .key("LinkId", AttributeValue::S(short_url.to_string()))
    //         .send()
    //         .await
    //         .map_err(|e| format!("Error getting item: {:?}", e))
    //         .map(|record| {
    //             record.item.and_then(|attributes| {
    //                 attributes
    //                     .get("OriginalLink")
    //                     .and_then(|v| v.as_s().cloned().ok())
    //             })
    //         })
    // }

    pub async fn retrieve_url_and_increment_clicks(
        &self,
        link_id: &str,
    ) -> Result<Option<String>, String> {
        let result = self
            .dynamodb_client
            .update_item()
            .table_name(&self.dynamodb_urls_table)
            .key("LinkId", AttributeValue::S(link_id.to_string()))
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
