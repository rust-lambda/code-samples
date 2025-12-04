use anyhow::Result;

use opentelemetry::{
    global,
    trace::TracerProvider,
};
use opentelemetry_appender_tracing::layer;
use opentelemetry_aws::detector::LambdaResourceDetector;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter};
use opentelemetry_resource_detectors::{OsResourceDetector, ProcessResourceDetector};
use opentelemetry_sdk::{
    logs::SdkLoggerProvider,
    metrics::SdkMeterProvider,
    propagation::TraceContextPropagator,
    resource::{
        EnvResourceDetector, ResourceDetector, SdkProvidedResourceDetector,
        TelemetryResourceDetector,
    },
    trace::{RandomIdGenerator, SdkTracerProvider},
    Resource,
};
use tracing::Level;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};

use tracing_subscriber::{prelude::*, EnvFilter};

// A Tracer Provider is a factory for Tracers
// A Tracer creates spans containing more information about what is happening for a given operation,
// such as a request in a service.
fn init_tracer() -> SdkTracerProvider {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("Failed to create span exporter");

    let lambda_detector = LambdaResourceDetector {};

    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(OsResourceDetector.detect())
        .with_resource(ProcessResourceDetector.detect())
        .with_resource(SdkProvidedResourceDetector.detect())
        .with_resource(EnvResourceDetector::new().detect())
        .with_resource(TelemetryResourceDetector.detect())
        .with_resource(lambda_detector.detect())
        .with_id_generator(RandomIdGenerator::default())
        .with_batch_exporter(exporter)
        .build();

    tracer_provider
}

// A Meter Provider is a factory for Meters
// A Meter creates metric instruments, capturing measurements about a service at runtime.
fn init_meter_provider() -> SdkMeterProvider {
    let exporter = MetricExporter::builder()
        .with_tonic()
        .build()
        .expect("Failed to create metric exporter");

    let lambda_detector = LambdaResourceDetector {};

    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_resource(OsResourceDetector.detect())
        .with_resource(ProcessResourceDetector.detect())
        .with_resource(SdkProvidedResourceDetector.detect())
        .with_resource(EnvResourceDetector::new().detect())
        .with_resource(TelemetryResourceDetector.detect())
        .with_resource(lambda_detector.detect())
        .build();

    global::set_meter_provider(meter_provider.clone());

    meter_provider
}

// A Logger Provider is a factory for Loggers
// The init_logger_provider function initialises a Logger Provider
// And sets up a Log Appender for the log crate, bridging logs to the OpenTelemetry Logger.
fn init_logger_provider() -> SdkLoggerProvider {
    // You can optionally use a stdout exporter to print logs to stdout instead of sending OTLP.
    let _stdout_exporter = opentelemetry_stdout::LogExporter::default();

    let otlp_exporter = LogExporter::builder()
        .with_tonic()
        .build()
        .expect("Failed to create log exporter");

    let lambda_detector = LambdaResourceDetector {};

    let logger_provider = SdkLoggerProvider::builder()
        .with_resource(
            Resource::builder()
                .with_service_name(
                    std::env::var("OTEL_SERVICE_NAME").unwrap_or("unknown-service".to_string()),
                )
                .build(),
        )
        .with_resource(OsResourceDetector.detect())
        .with_resource(ProcessResourceDetector.detect())
        .with_resource(SdkProvidedResourceDetector.detect())
        .with_resource(EnvResourceDetector::new().detect())
        .with_resource(TelemetryResourceDetector.detect())
        .with_resource(lambda_detector.detect())
        .with_simple_exporter(otlp_exporter)
        .build();

    logger_provider
}

pub fn init_otel() -> Result<OtelGuard> {
    let logger = init_logger_provider();
    let trace_provider = init_tracer();
    let meter = init_meter_provider();

    let tracer = trace_provider.tracer("tracing-otel-subscriber");

    let filter_otel = EnvFilter::new("info")
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("opentelemetry=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());
    let otel_layer = layer::OpenTelemetryTracingBridge::new(&logger).with_filter(filter_otel);

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::from_level(
            Level::INFO,
        ))
        .with(otel_layer)
        .with(MetricsLayer::new(meter.clone()))
        .with(OpenTelemetryLayer::new(tracer))
        .init();

    Ok(OtelGuard {
        tracer_provider: trace_provider,
        meter_provider: meter,
        logger_provider: logger,
    })
}

pub struct OtelGuard {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: SdkLoggerProvider,
}

impl OtelGuard {
    pub fn flush(&self) {
        if let Err(err) = self.tracer_provider.force_flush() {
            eprintln!("{err:?}");
        }
        if let Err(err) = self.meter_provider.force_flush() {
            eprintln!("{err:?}");
        }
        if let Err(err) = self.logger_provider.force_flush() {
            eprintln!("{err:?}");
        }
    }
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(err) = self.tracer_provider.shutdown() {
            eprintln!("{err:?}");
        }
        if let Err(err) = self.meter_provider.shutdown() {
            eprintln!("{err:?}");
        }
        if let Err(err) = self.logger_provider.shutdown() {
            eprintln!("{err:?}");
        }
    }
}
