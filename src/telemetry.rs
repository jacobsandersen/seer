use opentelemetry::{trace::TracerProvider, KeyValue};
use opentelemetry_otlp::{LogExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{logs::SdkLoggerProvider, trace::SdkTracerProvider, Resource};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Telemetry;

fn resource() -> Resource {
  Resource::builder()
    .with_attribute(KeyValue::new(SERVICE_NAME, "scribble"))
    .build()
}

fn init_tracer(otel_endpoint: &str) -> anyhow::Result<SdkTracerProvider> {
  let exporter = SpanExporter::builder()
    .with_tonic()
    .with_endpoint(otel_endpoint)
    .build()?;

  let provider = SdkTracerProvider::builder()
    .with_resource(resource())
    .with_batch_exporter(exporter)
    .build();

  Ok(provider)
}

fn init_logs(otel_endpoint: &str) -> anyhow::Result<SdkLoggerProvider> {
  let exporter = LogExporter::builder()
    .with_tonic()
    .with_endpoint(otel_endpoint)
    .build()?;

  let provider = SdkLoggerProvider::builder()
    .with_resource(resource())
    .with_batch_exporter(exporter)
    .build();

  Ok(provider)
}

pub fn init_telemetry(
  cfg: &Telemetry,
) -> anyhow::Result<Option<(SdkTracerProvider, SdkLoggerProvider)>> {
  if !cfg.enable {
    tracing_subscriber::fmt::init();
    return Ok(None);
  }

  let tracer = init_tracer(&cfg.otel_exporter_endpoint)?;
  let logger_provider = init_logs(&cfg.otel_exporter_endpoint)?;

  tracing_subscriber::registry()
    .with(EnvFilter::from_default_env())
    .with(tracing_subscriber::fmt::layer())
    .with(tracing_opentelemetry::layer().with_tracer(tracer.tracer("seer")))
    .with(opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider))
    .init();

  Ok(Some((tracer, logger_provider)))
}
