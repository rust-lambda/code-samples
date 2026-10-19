use std::sync::Arc;

use event_handler::function_handler;
use lambda_runtime::{run, service_fn, Error};

mod event_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let otel_guard =
        Arc::new(shared::observability::init_otel().expect("Failed to initialize telemetry"));

    run(service_fn(|evt| async {
        let res = function_handler(evt).await;

        otel_guard.flush();

        res
    }))
    .await
}
