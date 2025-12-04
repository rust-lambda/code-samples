use std::sync::Arc;

use crate::config::Config;
use crate::http_handler::{function_handler, HandlerDeps};
use lambda_http::{run, service_fn, tracing, Error};
use shared::adapters::DynamoDbUrlRepository;

mod config;
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let otel_guard = Arc::new(shared::observability::init_otel().expect("Failed to initialize telemetry"));
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let env = Config::load()?;
    let url_repo = DynamoDbUrlRepository::new(env.table_name, dynamodb_client);
    let deps = HandlerDeps { url_repo };

    run(service_fn(|event| async {
        let res = function_handler(&deps, event).await;

        otel_guard.flush();

        res
    })).await
}
