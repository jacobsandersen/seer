mod geo;
mod hardcover;
mod lastfm;
mod resp;
mod weather;

use axum::{
  Router, extract::{Request, State}, middleware::{self, Next}, response::Response, routing::{get, post}
};
use axum_extra::{TypedHeader, headers::{Authorization, authorization::Bearer}};
use tower_service::Service;
use worker::{Context, Env, HttpRequest, Result, event};

use crate::resp::{error, ok};

pub const CACHE_NS: &str = "seer_cache";
pub const GEO_DB: &str = "seer_geo";
pub const AUTH_SECRET_KEY: &str = "FIXED_AUTH";

#[derive(Debug, Clone)]
struct AppState {
  hardcover_key: String,
  lastfm_key: String,
  openweather_key: String,
  fixed_auth: String,
  cf: Env,
}

fn router(state: AppState) -> Router {
  Router::new()
    .route("/", get(root))
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
          Router::new()
            .route("/latest", get(geo::query::latest::handle))
        ),
    )
    .nest(
      "/weather",
      Router::new()
        .route("/conditions", get(weather::conditions::handle))
        .route("/pollution", get(weather::pollution::handle)),
    )
    .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
    .with_state(state)
}

async fn auth_middleware(State(state): State<AppState>, auth: TypedHeader<Authorization<Bearer>>, request: Request, next: Next) -> Response {
  if state.fixed_auth != auth.token() {
    return error("service authorization failed")
  }

  next.run(request).await
}

async fn root() -> Response {
  ok::<()>("success", None)
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
  let fixed_auth = env.secret(AUTH_SECRET_KEY)?.to_string();

  let state = AppState {
    hardcover_key,
    lastfm_key,
    openweather_key,
    fixed_auth,
    cf: env,
  };

  Ok(router(state).call(req).await?)
}
