mod geo;
mod hardcover;
mod lastfm;
mod resp;
mod weather;

use axum::{
  routing::{get, post},
  Router,
};
use tower_service::Service;
use worker::*;

pub const CACHE_NS: &str = "seer_cache";
pub const GEO_DB: &str = "seer_geo";

#[derive(Debug, Clone)]
struct AppState {
  hardcover_key: String,
  lastfm_key: String,
  #[allow(dead_code)]
  openweather_key: String,
  cf: Env,
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
        .route("/consume", post(geo::consume::handle))
        .nest(
          "/query",
          Router::new()
            .route("/latest", get(geo::query::latest::handle))
            .route("/more", get(geo::query::more::handle)),
        ),
    )
    .nest(
      "/weather",
      Router::new()
        .route("/conditions", get(weather::conditions::handle))
        .route("/pollution", get(weather::pollution::handle)),
    )
    .with_state(state)
}

#[event(fetch)]
async fn fetch(
  req: HttpRequest,
  env: Env,
  _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
  let hardcover_key = env.secret(hardcover::SECRET_KEY)?.to_string();
  let lastfm_key = env.secret(lastfm::SECRET_KEY)?.to_string();
  let openweather_key = env.secret(weather::SECRET_KEY)?.to_string();

  let state = AppState {
    hardcover_key,
    lastfm_key,
    openweather_key,
    cf: env,
  };

  Ok(router(state).call(req).await?)
}
