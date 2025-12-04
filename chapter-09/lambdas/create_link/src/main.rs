use std::sync::Arc;

use crate::config::Config;
use crate::event_publisher::SqsEventBridgePublisher;
use crate::http_handler::HandlerDeps;
use http_handler::function_handler;
use lambda_http::{run, service_fn, tracing, Error};
use shared::adapters::DynamoDbUrlRepository;
use shared::core::CuidGenerator;

mod config;
mod event_publisher;
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let otel_guard = Arc::new(shared::observability::init_otel().expect("Failed to initialize telemetry"));
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);
    let config = Config::load()?;
    let id_generator = CuidGenerator::new();
    let url_repo = DynamoDbUrlRepository::new(config.table_name, dynamodb_client);
    let event_publisher = SqsEventBridgePublisher::new(
        aws_sdk_sqs::Client::new(&aws_config),
        config.queue_url,
        aws_sdk_eventbridge::Client::new(&aws_config),
    );
    let deps = HandlerDeps {
        id_generator,
        url_repo,
        event_publisher,
    };

    run(service_fn(|event| async {
        let res = function_handler(&deps, event).await;

        otel_guard.flush();

        res
    })).await
}
