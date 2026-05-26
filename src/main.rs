use std::{process::exit, sync::Arc};

use axum::{
  extract::{Request, State},
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
use seer::{
  config::SeerConfig, db, geo, hardcover, lastfm, redis, resp, telemetry, weather, AppState,
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
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
    .route("/lastfm", get(lastfm::handle))
    .nest(
      "/hardcover",
      Router::new().route("/books", get(hardcover::handle)),
    )
    .nest(
      "/geo",
      Router::new()
        .route("/ingest", post(geo::ingest::handle))
        .nest(
          "/query",
          Router::new().route("/latest", get(geo::query::latest::handle)),
        ),
    )
    .nest(
      "/weather",
      Router::new()
        .route("/conditions", get(weather::conditions::handle))
        .route("/pollution", get(weather::pollution::handle)),
    )
    .layer(middleware::from_fn_with_state(
      state.clone(),
      auth_middleware,
    ))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  info!("loading configuration...");
  let config: Arc<SeerConfig> = Arc::new(
    Config::builder()
      .add_source(File::with_name("config").required(false))
      .add_source(Environment::default().separator("_"))
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
