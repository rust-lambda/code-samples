use crate::url_info::UrlDetails;
use async_trait::async_trait;
use cuid2::CuidConstructor;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[cfg(any(test, feature = "mocks"))]
use mockall::{automock, predicate::*};

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait UrlRepository: Debug {
    async fn get_url_from_short_link(&self, short_link: &str) -> Result<Option<String>, String>;
    async fn store_short_url(
        &self,
        url_to_shorten: String,
        short_link: String,
    ) -> Result<ShortUrl, String>;
    async fn add_details_to_short_url(
        &self,
        short_link: String,
        url_details: UrlDetails,
    ) -> Result<(), String>;
    async fn increment_clicks(&self, short_link: &str, n: u32) -> Result<(), String>;
    async fn list_urls(
        &self,
        last_evaluated_id: Option<String>,
    ) -> Result<(Vec<ShortUrl>, Option<String>), String>;
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait UrlInfo: Debug {
    async fn fetch_details(&self, url: &str) -> Result<UrlDetails, String>;
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait IdGenerator {
    fn generate_id(&self) -> String;
}

pub struct CuidGenerator {
    gen: CuidConstructor,
}

impl CuidGenerator {
    pub fn new() -> Self {
        Self {
            gen: CuidConstructor::new().with_length(10),
        }
    }
}

impl IdGenerator for CuidGenerator {
    fn generate_id(&self) -> String {
        self.gen.create_id()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ShortUrl {
    pub link_id: String,
    pub original_link: String,
    pub clicks: u32,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content_type: Option<String>,
}

impl ShortUrl {
    pub fn new(link_id: String, original_link: String) -> Self {
        Self {
            link_id,
            original_link,
            clicks: 0,
            title: None,
            description: None,
            content_type: None,
        }
    }
    pub fn with_details(
        link_id: String,
        original_link: String,
        clicks: u32,
        title: Option<String>,
        description: Option<String>,
        content_type: Option<String>,
    ) -> Self {
        Self {
            link_id,
            original_link,
            clicks,
            title,
            description,
            content_type,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListShortUrlsResponse {
    short_urls: Vec<ShortUrl>,
    last_evaluated_id: Option<String>,
}
