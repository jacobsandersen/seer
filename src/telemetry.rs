use std::{env, fs};

use anyhow::Context;
use opentelemetry::{global, trace::TracerProvider, Key};
use opentelemetry_otlp::{
  tonic_types::transport::{Certificate, ClientTlsConfig},
  LogExporter, SpanExporter, WithTonicConfig,
};
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

/// Mirrors the exporter's endpoint resolution (signal-specific env var, then
/// the generic one, then the plaintext gRPC default) to know whether the
/// channel will speak TLS.
fn resolves_to_https(signal_var: &str) -> bool {
  let explicit = |var: &str| env::var(var).ok().filter(|value| !value.is_empty());

  explicit(signal_var)
    .or_else(|| explicit("OTEL_EXPORTER_OTLP_ENDPOINT"))
    .is_some_and(|endpoint| endpoint.starts_with("https://"))
}

fn tls_config() -> anyhow::Result<ClientTlsConfig> {
  let mut tls = ClientTlsConfig::new().with_enabled_roots();

  if let Some(path) = env::var_os("OTEL_EXPORTER_OTLP_CERTIFICATE").filter(|path| !path.is_empty())
  {
    let pem =
      fs::read(&path).with_context(|| format!("failed to read {}", path.to_string_lossy()))?;
    tls = tls.ca_certificate(Certificate::from_pem(pem));
  }

  Ok(tls)
}

fn init_tracer() -> anyhow::Result<SdkTracerProvider> {
  let mut builder = SpanExporter::builder().with_tonic();
  if resolves_to_https("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT") {
    builder = builder.with_tls_config(tls_config()?);
  }
  let exporter = builder.build()?;

  let provider = SdkTracerProvider::builder()
    .with_resource(resource())
    .with_batch_exporter(exporter)
    .build();

  Ok(provider)
}

fn init_logs() -> anyhow::Result<SdkLoggerProvider> {
  let mut builder = LogExporter::builder().with_tonic();
  if resolves_to_https("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT") {
    builder = builder.with_tls_config(tls_config()?);
  }
  let exporter = builder.build()?;

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
