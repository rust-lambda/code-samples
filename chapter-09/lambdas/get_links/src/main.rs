use crate::config::Config;
use crate::http_handler::{function_handler, HandlerDeps};
use ::tracing::Instrument;
use lambda_http::{run, service_fn, tracing, Error};
use shared::adapters::DynamoDbUrlRepository;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

mod config;
mod http_handler;

static IS_COLD_START: AtomicBool = AtomicBool::new(true);

#[tokio::main]
async fn main() -> Result<(), Error> {
    let otel_guard =
        Arc::new(shared::observability::init_otel().expect("Failed to initialize telemetry"));
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let env = Config::load()?;
    let url_repo = DynamoDbUrlRepository::new(env.table_name, dynamodb_client);
    let deps = HandlerDeps { url_repo };

    run(service_fn(|event| async {
        let was_cold_start = IS_COLD_START.swap(false, Ordering::SeqCst);

        let handler_span = tracing::info_span!(
            "aws.lambda",
            operation_name = "aws.lambda",
            faas.coldstart = was_cold_start,
            cloud.provider = "aws",
            event_type = "http"
        );

        let res = function_handler(&deps, event)
            .instrument(handler_span)
            .await;

        otel_guard.flush();

        res
    }))
    .await
}
