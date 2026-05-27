use std::{process::exit, sync::Arc};

use axum::{
  extract::{Request, State},
  http,
  middleware::{self, Next},
  response::Response,
  routing::{get, post},
  Router,
};
use axum_extra::{
  headers::{authorization::Bearer, Authorization},
  TypedHeader,
};
use axum_prometheus::{metrics_exporter_prometheus::PrometheusHandle, PrometheusMetricLayer};
use config::{Config, Environment, File};
use opentelemetry::global;
use opentelemetry_http::HeaderExtractor;
use seer::{
  config::SeerConfig, db, geo, hardcover, lastfm, redis, resp, telemetry, weather, AppState,
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{error, info, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use validator::Validate;

async fn auth_middleware(
  State(state): State<AppState>,
  auth: TypedHeader<Authorization<Bearer>>,
  request: Request,
  next: Next,
) -> Response {
  if state.config.fixed_auth != auth.token() {
    return resp::error("service authorization failed");
  }

  next.run(request).await
}

fn public_router(state: AppState) -> (Router, PrometheusHandle) {
  let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

  let router = Router::new()
    .route("/lastfm", get(lastfm::now_playing))
    .nest(
      "/hardcover",
      Router::new().route("/books", get(hardcover::books)),
    )
    .nest(
      "/geo",
      Router::new()
        .route("/ingest", post(geo::ingest::ingest))
        .nest(
          "/query",
          Router::new().route("/latest", get(geo::query::latest::latest_location)),
        ),
    )
    .nest(
      "/weather",
      Router::new()
        .route("/conditions", get(weather::conditions::current_conditions))
        .route("/pollution", get(weather::pollution::current_pollution)),
    )
    .layer(middleware::from_fn_with_state(
      state.clone(),
      auth_middleware,
    ))
    .layer(
      TraceLayer::new_for_http().make_span_with(|req: &http::Request<_>| {
        let parent_cx =
          global::get_text_map_propagator(|prop| prop.extract(&HeaderExtractor(req.headers())));

        let ua = req
          .headers()
          .get("user-agent")
          .and_then(|v| v.to_str().ok())
          .unwrap_or("unknown");

        let ip = req
          .headers()
          .get("x-forwarded-for")
          .or(req.headers().get("x-real-ip"))
          .and_then(|v| v.to_str().ok())
          .unwrap_or("unknown");

        let span = info_span!(
          "http.request",
          method = %&req.method(),
          uri = %&req.uri(),
          user_agent = %ua,
          client_ip = %ip
        );

        let _ = span.set_parent(parent_cx);
        span
      }),
    )
    .layer(prometheus_layer)
    .with_state(state);

  (router, metric_handle)
}

fn metric_router(metric_handle: PrometheusHandle) -> Router {
  Router::new().route("/metrics", get(|| async move { metric_handle.render() }))
}

async fn serve(app: Router, binding: &str) {
  let listener = TcpListener::bind(binding)
    .await
    .expect(&format!("failed to bind: {binding}"));
  axum::serve(listener, app).await.unwrap();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  info!("loading configuration...");
  let config: Arc<SeerConfig> = Arc::new(
    Config::builder()
      .add_source(File::with_name("config").required(false))
      .add_source(Environment::default().separator("__"))
      .build()?
      .try_deserialize()?,
  );

  info!("validating configuration...");
  match config.validate() {
    Ok(_) => (),
    Err(e) => {
      error!("failed to validate configuration: {e:?}");
      exit(1);
    }
  }

  info!("initialize telemetry...");
  let telemetry = telemetry::init_telemetry(&config.telemetry)?;

  info!("establishing redis connection...");
  let redis = redis::initialize_redis(&config.redis).await?;

  info!("establishing database connection...");
  let db = db::initialize_db(&config.db).await?;

  info!("creating app state...");
  let binding = &config.binding.to_string();
  let metrics_binding = &config.metrics_binding.to_string();
  let state = AppState { config, redis, db };

  info!("setting up routes...");
  let (public, metric_handle) = public_router(state);
  let metrics = metric_router(metric_handle);

  info!("seer is listening on {binding} (metrics on {metrics_binding})");

  tokio::join!(serve(public, binding), serve(metrics, metrics_binding));

  info!("seer is shutting down...");

  if let Some((tracer, logger)) = telemetry {
    info!("Shutting down tracer...");
    let _ = tracer.shutdown();

    info!("Shutting down logger...");
    let _ = logger.shutdown();
  }

  info!("goodbye");
  Ok(())
}
