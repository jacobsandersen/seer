use std::env;

use opentelemetry::{global, trace::TracerProvider, Key};
use opentelemetry_otlp::{LogExporter, SpanExporter};
use opentelemetry_sdk::{
  logs::SdkLoggerProvider,
  propagation::TraceContextPropagator,
  resource::{EnvResourceDetector, ResourceDetector},
  trace::SdkTracerProvider,
  Resource,
};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Telemetry;

fn resource() -> Resource {
  let service_name_from_env = env::var("OTEL_SERVICE_NAME")
    .map(|v| !v.is_empty())
    .unwrap_or(false)
    || EnvResourceDetector::new()
      .detect()
      .get_ref(&Key::new(SERVICE_NAME))
      .is_some();

  let mut builder = Resource::builder();
  if !service_name_from_env {
    builder = builder.with_service_name("seer");
  }
  builder.build()
}

fn init_tracer() -> anyhow::Result<SdkTracerProvider> {
  let exporter = SpanExporter::builder().with_http().build()?;

  let provider = SdkTracerProvider::builder()
    .with_resource(resource())
    .with_batch_exporter(exporter)
    .build();

  Ok(provider)
}

fn init_logs() -> anyhow::Result<SdkLoggerProvider> {
  let exporter = LogExporter::builder().with_http().build()?;

  let provider = SdkLoggerProvider::builder()
    .with_resource(resource())
    .with_batch_exporter(exporter)
    .build();

  Ok(provider)
}

pub fn init_telemetry(
  cfg: &Telemetry,
) -> anyhow::Result<Option<(SdkTracerProvider, SdkLoggerProvider)>> {
  global::set_text_map_propagator(TraceContextPropagator::new());

  if !cfg.enable {
    tracing_subscriber::fmt::init();
    return Ok(None);
  }

  let tracer = init_tracer()?;
  let logger_provider = init_logs()?;

  tracing_subscriber::registry()
    .with(EnvFilter::from_default_env())
    .with(tracing_subscriber::fmt::layer())
    .with(tracing_opentelemetry::layer().with_tracer(tracer.tracer("seer")))
    .with(opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider))
    .init();

  Ok(Some((tracer, logger_provider)))
}
