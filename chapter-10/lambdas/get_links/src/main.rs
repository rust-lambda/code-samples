use crate::config::Config;
use crate::http_handler::{function_handler, HandlerDeps};
use lambda_http::tower::ServiceBuilder;
use lambda_http::{http, run, service_fn, Body, Error};
use shared::adapters::DynamoDbUrlRepository;
use shared::middleware::observability::HttpObservabilityLayer;
use shared::middleware::rate_limit::{RateLimitConfig, RateLimitLayer};
use std::sync::Arc;
use std::time::Duration;

mod config;
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let otel_guard =
        Arc::new(shared::observability::init_otel().expect("Failed to initialize telemetry"));
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let env = Config::load()?;
    let url_repo = DynamoDbUrlRepository::new(env.table_name, dynamodb_client.clone());
    let observability_layer = HttpObservabilityLayer::new(Arc::clone(&otel_guard));
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
        .layer(observability_layer)
        .layer(rate_limit_layer)
        .service(service_fn(move |event: http::Request<Body>| {
            let deps = Arc::clone(&deps);
            async move { function_handler(deps.as_ref(), event).await }
        }));

    run(service).await
}
