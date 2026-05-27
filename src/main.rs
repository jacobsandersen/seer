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

fn router(state: AppState) -> Router {
  Router::new()
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

        let span = info_span!("http.request", method = %&req.method(), uri = %&req.uri());
        let _ = span.set_parent(parent_cx);
        span
      }),
    )
    .with_state(state)
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
  let state = AppState { config, redis, db };

  info!("setting up routes...");
  let router = router(state);

  info!("binding tcp listener...");
  let listener = TcpListener::bind(binding)
    .await
    .expect("Failed to bind TCP listener");

  info!("seer is listening on {binding}");

  let _ = axum::serve(listener, router).await;

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
