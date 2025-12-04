use std::sync::Arc;

use lambda_runtime::{run, service_fn, tracing, Error};
mod event_handler;
use event_handler::function_handler;
use shared::{adapters::DynamoDbUrlRepository, url_info::HttpUrlInfo};

use crate::event_handler::HandlerDeps;

mod config;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let otel_guard =
        Arc::new(shared::observability::init_otel().expect("Failed to initialize telemetry"));
    
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);
    let config = config::Config::load()?;

    let url_repo = DynamoDbUrlRepository::new(config.table_name, dynamodb_client);
    let http_client = shared::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    let url_info = HttpUrlInfo::new(http_client.clone());

    let handler_deps = HandlerDeps { url_repo, url_info };

    run(service_fn(|event| async {
        let res = function_handler(&handler_deps, event).await;

        otel_guard.flush();

        res
    })).await
}
