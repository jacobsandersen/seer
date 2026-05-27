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

#[derive(Debug, Clone)]
pub struct AppState {
  pub config: Arc<SeerConfig>,
  pub redis: ::redis::aio::ConnectionManager,
  pub db: PgPool,
}
