use crate::{configuration::Configuration, url_info::UrlDetails};
use async_trait::async_trait;
use cuid2::CuidConstructor;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[cfg(any(test, feature = "mocks"))]
use mockall::{automock, predicate::*};

#[derive(Serialize, Deserialize)]
pub struct ShortenUrlRequest {
    url_to_shorten: String,
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait UrlRepository: Debug {
    async fn get_url_from_short_link(
        &self,
        short_link: &str,
    ) -> Result<Option<String>, String>;
    async fn store_short_url(
        &self,
        url_to_shorten: String,
        short_link: String,
        url_details: UrlDetails,
    ) -> Result<ShortUrl, String>;
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

#[derive(Debug)]
pub struct UrlShortener<R: UrlRepository, I: UrlInfo> {
    url_repo: R,
    url_info: I,
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
    pub fn new(
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

impl<R: UrlRepository, I: UrlInfo> UrlShortener<R, I> {
    pub fn new(url_repo: R, url_info: I) -> Self {
        Self { url_repo, url_info }
    }

    pub async fn shorten_url(
        &self,
        req: ShortenUrlRequest,
    ) -> Result<ShortUrl, String> {
        let short_url = self.generate_short_url();
        let url_details = self
            .url_info
            .fetch_details(&req.url_to_shorten)
            .await
            .unwrap_or_default();

        self.url_repo
            .store_short_url( req.url_to_shorten.clone(), short_url, url_details)
            .await
    }

    pub async fn retrieve_url_and_increment_clicks(
        &self,
        link_id: &str,
    ) -> Result<Option<String>, String> {
        self.url_repo.get_url_from_short_link( link_id).await
    }

    pub async fn list_urls(
        &self,
        last_evaluated_id: Option<String>,
    ) -> Result<ListShortUrlsResponse, String> {
        let (short_urls, last_evaluated_id) =
            self.url_repo.list_urls(last_evaluated_id).await?;

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

#[cfg(test)]
mod tests {
    use mockall::predicate;

    use super::*;

    #[tokio::test]
    async fn when_valid_link_is_passed_should_retrieve_info_and_store() {
        let test_url = "https://google.com";
        let test_title = "Google".to_string();
        let test_description = "Test description".to_string();
        let test_content_type = "text/html".to_string();

        let mut url_repo = MockUrlRepository::new();
        url_repo
            .expect_store_short_url()
            .with(
                predicate::eq(test_url.to_string()),
                predicate::always(),
                predicate::always(),
            )
            .times(1)
            .returning(|url_to_shorten, short_url, url_details| {
                Ok(ShortUrl::new(
                    short_url,
                    url_to_shorten,
                    0,
                    url_details.title,
                    url_details.description,
                    url_details.content_type,
                ))
            });

        let mut mock_url_info = MockUrlInfo::new();
        mock_url_info
            .expect_fetch_details()
            .with(predicate::eq(test_url.to_string()))
            .times(1)
            .returning(move |_url| {
                Ok(UrlDetails {
                    title: Some(test_title.clone()),
                    description: Some(test_description.clone()),
                    content_type: Some(test_content_type.clone()),
                })
            });

        let url_shortener = UrlShortener::new(url_repo, mock_url_info);

        let result = url_shortener
            .shorten_url(ShortenUrlRequest {
                url_to_shorten: test_url.to_string(),
            })
            .await;

        assert!(result.is_ok());

        let short_url = result.unwrap();
        assert_eq!(short_url.original_link, test_url);
        assert_eq!(short_url.title.unwrap(), "Google".to_string());
        assert_eq!(
            short_url.description.unwrap(),
            "Test description".to_string()
        );
        assert_eq!(short_url.content_type.unwrap(), "text/html".to_string());
    }

    #[tokio::test]
    async fn on_valid_call_should_search_and_return() {
        let test_short_url = "a-random-id";

        let mut url_repo = MockUrlRepository::new();
        url_repo
            .expect_get_url_from_short_link()
            .with(predicate::eq(test_short_url.to_string()))
            .times(1)
            .returning(|_url_to_shorten| Ok(Some("https://google.com".to_string())));

        let mock_url_info = MockUrlInfo::new();

        let url_shortener = UrlShortener::new(url_repo, mock_url_info);

        let result = url_shortener
            .retrieve_url_and_increment_clicks(test_short_url)
            .await;

        assert!(result.is_ok());

        let short_url = result.unwrap();
        assert_eq!(short_url, Some("https://google.com".to_string()));
    }

    #[tokio::test]
    async fn on_url_not_found_call_should_return_error() {
        let test_short_url = "a-random-id";

        let mut url_repo = MockUrlRepository::new();
        url_repo
            .expect_get_url_from_short_link()
            .with(predicate::eq(test_short_url.to_string()))
            .times(1)
            .returning(|_url_to_shorten| Err("Not found".to_string()));

        let mock_url_info = MockUrlInfo::new();

        let url_shortener = UrlShortener::new(url_repo, mock_url_info);

        let result = url_shortener
            .retrieve_url_and_increment_clicks(test_short_url)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn on_list_urls_should_return_vec() {
        let mut url_repo = MockUrlRepository::new();
        url_repo
            .expect_list_urls()
            .with(predicate::eq(None))
            .times(1)
            .returning(|_previous_token| {
                Ok((
                    vec![ShortUrl::new(
                        "horytla".to_string(),
                        "https://google.com".to_string(),
                        0,
                        None,
                        None,
                        None,
                    )],
                    None,
                ))
            });

        let mock_url_info = MockUrlInfo::new();

        let url_shortener = UrlShortener::new(url_repo, mock_url_info);

        let result = url_shortener.list_urls(None).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn on_list_urls_error_should_pass_up() {
        let mut url_repo = MockUrlRepository::new();
        url_repo
            .expect_list_urls()
            .with(predicate::eq(None))
            .times(1)
            .returning(|_url_to_shorten| Err("Error".to_string()));

        let mock_url_info = MockUrlInfo::new();

        let url_shortener = UrlShortener::new(url_repo, mock_url_info);

        let result = url_shortener.list_urls(None).await;

        assert!(result.is_err());
    }
}
