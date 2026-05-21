mod lastfm;
mod hardcover;
mod resp;

use axum::{Router, http::HeaderValue, routing::get};
use tower_service::Service;
use worker::*;

const CACHE_NS: &str = "seer_cache";

#[derive(Debug, Clone)]
struct AppState {
  hardcover_key: String,
  lastfm_key: String,
  kv: KvStore
}

fn router(state: AppState) -> Router {
    Router::new()
      .nest(
        "/lastfm", 
        Router::new()
          .route("/now", get(lastfm::now_playing::handle))
      )
      .nest(
        "/hardcover",
          Router::new()
            .route("/now", get(hardcover::now_reading::handle))
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
      kv
    };

    let mut resp = router(state).call(req).await?;

    let cors_headers = [
      ("Access-Control-Allow-Origin", "*"),
      ("Access-Control-Allow-Methods", "GET")
    ];

    let headers = resp.headers_mut();
    for (key, value) in cors_headers {
      headers.append(key, HeaderValue::from_str(value)?);
    }

    Ok(resp)
}
