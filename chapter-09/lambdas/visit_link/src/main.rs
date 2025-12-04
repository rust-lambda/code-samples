use std::sync::Arc;

use crate::config::Config;
use crate::http_handler::HandlerDeps;
use http_handler::function_handler;
use lambda_http::{run, service_fn, Error};
use shared::adapters::DynamoDbUrlRepository;

mod config;
mod event_publisher;
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let otel_guard =
        Arc::new(shared::observability::init_otel().expect("Failed to initialize telemetry"));

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);
    let kinesis_client = aws_sdk_kinesis::Client::new(&aws_config);
    let config = Config::load()?;
    let url_repo = DynamoDbUrlRepository::new(config.table_name, dynamodb_client);
    let event_publisher =
        event_publisher::KinesisEventPublisher::new(kinesis_client, config.stream_name);
    let deps = HandlerDeps {
        url_repo,
        event_publisher,
    };

    run(service_fn(|event| async {
        let res = function_handler(&deps, event).await;

        otel_guard.flush();

        res
    }))
    .await
}
