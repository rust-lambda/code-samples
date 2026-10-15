use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use crate::event_publisher::SqsEventBridgePublisher;
use crate::http_handler::HandlerDeps;
use http_handler::function_handler;
use lambda_http::tower::ServiceBuilder;
use lambda_http::{http, run, service_fn, Body, Error};
use shared::adapters::DynamoDbUrlRepository;
use shared::core::CuidGenerator;
use shared::middleware::observability::HttpObservabilityLayer;
use shared::middleware::rate_limit::{RateLimitConfig, RateLimitLayer};

mod config;
mod event_publisher;
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let otel_guard =
        Arc::new(shared::observability::init_otel().expect("Failed to initialize telemetry"));
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);
    let config = Config::load()?;
    let id_generator = CuidGenerator::new();
    let url_repo = DynamoDbUrlRepository::new(config.table_name, dynamodb_client.clone());
    let observability_layer = HttpObservabilityLayer::new(Arc::clone(&otel_guard));
    let rate_limit_layer = RateLimitLayer::new(
        RateLimitConfig {
            table_name: config.rate_limit_table_name,
            max_requests: config.rate_limit_max_requests,
            window_duration: Duration::from_secs(config.rate_limit_window_secs),
        },
        dynamodb_client,
    );
    let event_publisher = SqsEventBridgePublisher::new(
        aws_sdk_sqs::Client::new(&aws_config),
        config.queue_url,
        aws_sdk_eventbridge::Client::new(&aws_config),
    );
    let deps = Arc::new(HandlerDeps {
        id_generator,
        url_repo,
        event_publisher,
    });

    let service = ServiceBuilder::new()
        .layer(observability_layer)
        .layer(rate_limit_layer)
        .service(service_fn(move |event: http::Request<Body>| {
            let deps = Arc::clone(&deps);
            async move { function_handler(deps.as_ref(), event).await }
        }));

    run(service).await
}
