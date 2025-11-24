#[cfg(test)]
use mockall::automock;
use shared::core::ShortUrl;

#[cfg_attr(test, automock)]
pub(crate) trait EventPublisher {
    async fn publish_link_clicked(
        &self,
        short_url: &ShortUrl,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub(crate) struct KinesisEventPublisher {
    pub kinesis_client: aws_sdk_kinesis::Client,
    pub stream_name: String,
}

impl KinesisEventPublisher {
    pub fn new(kinesis_client: aws_sdk_kinesis::Client, stream_name: String) -> Self {
        Self {
            kinesis_client,
            stream_name,
        }
    }
}

impl EventPublisher for KinesisEventPublisher {
    async fn publish_link_clicked(
        &self,
        short_url: &ShortUrl,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = serde_json::to_vec(short_url)?;

        self.kinesis_client
            .put_record()
            .stream_name(&self.stream_name)
            .partition_key(&short_url.link_id)
            .data(data.into())
            .send()
            .await?;

        Ok(())
    }
}
