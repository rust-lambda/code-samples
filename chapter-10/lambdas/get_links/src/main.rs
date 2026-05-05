use crate::config::Config;
use crate::http_handler::{function_handler, HandlerDeps};
use ::tracing::Instrument;
use lambda_http::tower::ServiceBuilder;
use lambda_http::{http, run, service_fn, tracing, Body, Error};
use shared::adapters::DynamoDbUrlRepository;
use shared::middleware::rate_limit::{RateLimitConfig, RateLimitLayer};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

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
    let url_repo = DynamoDbUrlRepository::new(env.table_name, dynamodb_client.clone());
    let rate_limit_layer = RateLimitLayer::new(
        RateLimitConfig {
            table_name: env.rate_limit_table_name,
            max_requests: env.rate_limit_max_requests,
            window_duration: Duration::from_secs(env.rate_limit_window_secs),
        },
        dynamodb_client,
    );
    let deps = Arc::new(HandlerDeps { url_repo });

    let service = ServiceBuilder::new()
        .layer(rate_limit_layer)
        .service(service_fn({
            let deps = Arc::clone(&deps);
            let otel_guard = Arc::clone(&otel_guard);

            move |event: http::Request<Body>| {
                let deps = Arc::clone(&deps);
                let otel_guard = Arc::clone(&otel_guard);

                async move {
                    let was_cold_start = IS_COLD_START.swap(false, Ordering::SeqCst);

                    let handler_span = tracing::info_span!(
                        "aws.lambda",
                        operation_name = "aws.lambda",
                        faas.coldstart = was_cold_start,
                        cloud.provider = "aws",
                        event_type = "http"
                    );

                    let res = function_handler(deps.as_ref(), event)
                        .instrument(handler_span)
                        .await;

                    otel_guard.flush();

                    res
                }
            }
        }));

    run(service).await
}
