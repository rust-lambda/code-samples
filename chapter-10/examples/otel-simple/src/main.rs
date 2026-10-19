use shared::observability::init_otel;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() {
    // Telemetry is flushed on drop
    let _otel_guard = init_otel().expect("Failed to initialize telemetry");

    do_work();
}

#[tracing::instrument()]
fn do_work() {
    // Simulate some work being done
    std::thread::sleep(std::time::Duration::from_millis(500));

    let my_random_variable = "this is the value";

    error!("the random variable value is {}.", my_random_variable);
    warn!("This is a warning log");
    info!("Work completed");

    do_some_more_work();
}

#[tracing::instrument()]
fn do_some_more_work() {
    std::thread::sleep(std::time::Duration::from_millis(1500));
}
