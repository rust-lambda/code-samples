use cloudevents::{AttributesReader, EventBuilder, EventBuilderV10};
#[cfg(test)]
use mockall::automock;
use shared::core::{CuidGenerator, IdGenerator, ShortUrl};

type Error = Box<dyn std::error::Error + Send + Sync>;

#[cfg_attr(test, automock)]
pub(crate) trait EventPublisher {
    async fn publish_link_clicked(&self, short_url: &ShortUrl) -> Result<(), Error>;
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
    #[tracing::instrument("publish link_clicked.v1", skip(self, short_url), fields(
    messaging.message.id = tracing::field::Empty,
    messaging.operation.name = "publish",
    messaging.destination = "aws_kinesis",
    messaging.client.id = "visit_link",
))]
    async fn publish_link_clicked(&self, short_url: &ShortUrl) -> Result<(), Error> {
        let current_span = tracing::Span::current();
        let data = serde_json::to_vec(short_url)?;
        let trace_parent = shared::observability::get_traceparent_extension_value(&current_span);

        let event: cloudevents::Event = EventBuilderV10::new()
            .id(CuidGenerator::new().generate_id().to_string())
            .ty("rust-link-shortener")
            .source("http://rust-link-shortener.com")
            .data("application/json", data)
            .extension("traceparent", trace_parent)
            .build()
            .unwrap();
        tracing::Span::current().record("messaging.message.id", &event.id().to_string());

        let data = serde_json::to_vec(&event)?;

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
