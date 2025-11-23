use crate::env_vars::EnvVars;
use crate::event_publisher::SqsEventBridgePublisher;
use crate::http_handler::HandlerDeps;
use http_handler::function_handler;
use lambda_http::{run, service_fn, tracing, Error};
use shared::adapters::DynamoDbUrlRepository;
use shared::core::CuidGenerator;

mod env_vars;
mod event_publisher;
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let env = EnvVars::load()?;
    let id_generator = CuidGenerator::new();
    let url_repo = DynamoDbUrlRepository::new(env.table_name, dynamodb_client);
    let event_publisher = SqsEventBridgePublisher::new(
        aws_sdk_sqs::Client::new(&config),
        env.queue_url,
        aws_sdk_eventbridge::Client::new(&config),
    );
    let deps = HandlerDeps {
        id_generator,
        url_repo,
        event_publisher,
    };

    run(service_fn(|event| function_handler(&deps, event))).await
}
