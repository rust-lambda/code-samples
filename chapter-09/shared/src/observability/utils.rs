use cloudevents::Event;
use opentelemetry::{
    Context, SpanId, TraceFlags, TraceId, trace::{SpanContext, TraceState}
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub fn get_traceparent_extension_value(span: &tracing::Span) -> String {
    use opentelemetry::trace::TraceContextExt;

    let binding = span.context();
    let otel_span = binding.span();
    let span_context = otel_span.span_context();

    let trace_id = span_context.trace_id().to_string();
    let span_id = span_context.span_id().to_string();
    let trace_flags = span_context.trace_flags();

    format!("00-{}-{}-{:02x}", trace_id, span_id, trace_flags.to_u8())
}

pub fn add_span_link_from(span: &tracing::Span, cloud_event: &Event) {
    use opentelemetry::trace::TraceContextExt;

    let trace_parent = cloud_event.extension("traceparent");

    let trace_parent = match trace_parent {
        Some(value) => {
            tracing::info!("Extracted traceparent: {:?}", value);
            value.to_string()
        }
        None => {
            tracing::info!("No traceparent found in CloudEvent");
            return;
        }
    };

    let current_binding = span.context();
    let current_otel_span = current_binding.span();

    let remote_context = extract_span_context_from(&trace_parent);

    match remote_context {
        Some(remote_span_context) => {
            current_otel_span.add_link(remote_span_context, vec![]);
        }
        None => {
            tracing::warn!(
                "Failed to extract span context from traceparent: {}",
                trace_parent
            );
        }
    }
}

pub fn add_parent_context_from(span: &tracing::Span, cloud_event: &Event) {
    use opentelemetry::trace::TraceContextExt;

    let trace_parent = cloud_event.extension("traceparent");

    let trace_parent = match trace_parent {
        Some(value) => {
            tracing::info!("Extracted traceparent: {:?}", value);
            value.to_string()
        }
        None => {
            tracing::info!("No traceparent found in CloudEvent");
            return;
        }
    };

    let remote_context = extract_span_context_from(&trace_parent);

    match remote_context {
        Some(remote_span_context) => {
            let _ = span.set_parent(Context::new().with_remote_span_context(remote_span_context.clone()));
        }
        None => {
            tracing::warn!(
                "Failed to extract span context from traceparent: {}",
                trace_parent
            );
        }
    }
}

/// Generate a span context from the trace_id and span_id fields
fn extract_span_context_from(trace_parent: &str) -> Option<SpanContext> {
    let trace_parts: Vec<&str> = trace_parent.split("-").collect();

    if trace_parts.len() < 4 {
        return None;
    }

    let trace_id = TraceId::from_hex(trace_parts[1]);

    match trace_id {
        Ok(trace_id) => {
            let span_id =
                SpanId::from_hex(trace_parts[2]).unwrap_or_else(|_| SpanId::from_bytes([0u8; 8]));

            let span_context = SpanContext::new(
                trace_id,
                span_id,
                TraceFlags::SAMPLED,
                false,
                TraceState::NONE,
            );

            Some(span_context)
        }
        Err(_) => None,
    }
}
