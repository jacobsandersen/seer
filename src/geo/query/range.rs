use axum::{
  extract::{Query, State},
  response::Response,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, instrument};

use crate::{
  resp::{error, ok},
  AppState,
};

#[derive(Debug, Deserialize)]
pub struct RequestOpts {
  start: chrono::DateTime<Utc>,
  end: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct LocationEntry {
  pub lon: f64,
  pub lat: f64,
  pub recorded_at: chrono::DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum LocationsInRangeError {
  #[error("sqlx error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

#[instrument(skip(state))]
pub async fn get_locations_in_range(
  state: AppState,
  start: chrono::DateTime<Utc>,
  end: chrono::DateTime<Utc>,
) -> Result<Vec<LocationEntry>, LocationsInRangeError> {
  let data: Vec<LocationEntry> =
    sqlx::query_as("select st_x(geom) as lon, st_y(geom) as lat, recorded_at from where_was_i_between($1, $2)")
      .bind(start)
      .bind(end)
      .fetch_all(&state.db)
      .await?;

  Ok(data)
}

#[instrument(skip(state))]
pub async fn locations_in_range(
  State(state): State<AppState>,
  opts: Query<RequestOpts>,
) -> Response {
  match get_locations_in_range(state, opts.start, opts.end).await {
    Ok(data) => ok("success", Some(data)),
    Err(e) => {
      error!("failed to query data: {e:?}");
      error("location range query failed")
    }
  }
}
