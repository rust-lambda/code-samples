mod configuration;
mod lambda_instrumentation;
mod utils;

pub use configuration::{init_otel, OtelGuard};
pub use utils::{get_traceparent_extension_value, add_span_link_from};