mod hardcover;
mod lastfm;
mod resp;

use axum::{routing::get, Router};
use tower_service::Service;
use worker::*;

const CACHE_NS: &str = "seer_cache";

#[derive(Debug, Clone)]
struct AppState {
  hardcover_key: String,
  lastfm_key: String,
  kv: KvStore,
}

fn router(state: AppState) -> Router {
  Router::new()
    .nest(
      "/lastfm",
      Router::new().route("/now", get(lastfm::now_playing::handle)),
    )
    .nest(
      "/hardcover",
      Router::new().route("/books", get(hardcover::handle)),
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
  let kv = env.kv(CACHE_NS)?;

  let state = AppState {
    hardcover_key,
    lastfm_key,
    kv,
  };

  Ok(router(state).call(req).await?)
}
