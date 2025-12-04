use crate::event_handler::HandlerDeps;
use event_handler::function_handler;
use lambda_runtime::{run, service_fn, tracing, Error};
use shared::adapters::DynamoDbUrlRepository;

mod config;
mod event_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);
    let config = config::Config::load()?;
    let url_repo = DynamoDbUrlRepository::new(config.table_name, dynamodb_client);
    let handler_deps = HandlerDeps { url_repo };

    run(service_fn(|event| function_handler(&handler_deps, event))).await
}
