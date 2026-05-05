mod configuration;
mod lambda_instrumentation;
mod utils;

pub use configuration::{init_otel, OtelGuard};
pub use utils::{add_parent_context_from, add_span_link_from, get_traceparent_extension_value};
