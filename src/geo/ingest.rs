use axum::{
  extract::State,
  response::{IntoResponse, Response},
  Json,
};
use geojson::GeoJson;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::error;

use crate::{resp::error, AppState};

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
  #[error("serde error: {0}")]
  Serde(#[from] serde_json::Error),

  #[error("sqlx error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trip {
  distance: u32,
  mode: String,
  current_location: GeoJson,
  start_location: GeoJson,
  start: chrono::DateTime<chrono::Local>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngestionPayload {
  locations: Vec<GeoJson>,
  current: Option<GeoJson>,
  trip: Option<Trip>,
}

async fn ingest_batch(pool: &sqlx::PgPool, payload: IngestionPayload) -> Result<(), IngestError> {
  let serialized: Vec<serde_json::Value> = payload
    .locations
    .iter()
    .map(|loc| serde_json::to_value(loc))
    .collect::<Result<Vec<_>, _>>()?;

  let mut tx = pool.begin().await?;
  let mut qb = sqlx::QueryBuilder::new("insert into geodata(data) ");

  qb.push_values(&serialized, |mut b, location| {
    b.push_bind(location);
  });

  qb.build().execute(&mut *tx).await?;
  tx.commit().await?;
  Ok(())
}

pub async fn handle(
  State(state): State<AppState>,
  Json(payload): Json<IngestionPayload>,
) -> Response {
  match ingest_batch(&state.db, payload).await {
    Ok(()) => (axum::http::StatusCode::OK, Json(json!({"result": "ok"}))).into_response(),
    Err(e) => {
      error!("batch ingestion error: {e:?}");
      error(&format!("failed to ingest batch"))
    }
  }
}
