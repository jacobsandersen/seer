pub mod config;
pub mod db;
pub mod geo;
pub mod hardcover;
pub mod lastfm;
pub mod redis;
pub mod resp;
pub mod telemetry;
pub mod weather;

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::SeerConfig;

pub const CACHE_NS: &str = "seer_cache";
pub const GEO_DB: &str = "seer_geo";
pub const AUTH_SECRET_KEY: &str = "FIXED_AUTH";

#[derive(Debug, Clone)]
pub struct AppState {
  pub config: Arc<SeerConfig>,
  pub redis: ::redis::aio::ConnectionManager,
  pub db: PgPool,
}
