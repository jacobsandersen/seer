use axum::{extract::State, response::Response};
use chrono::Duration;
use geojson::GeoJson;
use tracing::{error, instrument};

use crate::{
  redis::JsonExt,
  resp::{error, ok},
  AppState,
};

#[derive(Debug, thiserror::Error)]
pub enum LatestLocationError {
  #[error("serde error occurred: {0}")]
  Serde(#[from] serde_json::Error),

  #[error("sqlx error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

#[instrument]
pub async fn get_latest_location(
  state: &mut AppState,
) -> Result<Option<GeoJson>, LatestLocationError> {
  let cache_key = "geo_latest_pos";

  if let Ok(Some(pos)) = state.redis.get_json::<GeoJson>(cache_key).await {
    return Ok(Some(pos));
  }

  let data: Option<(String,)> = sqlx::query_as("select data from geodata order by ts desc limit 1")
    .fetch_optional(&state.db)
    .await?;

  match data {
    None => Ok(None),
    Some(location) => {
      let location = serde_json::from_str::<GeoJson>(&location.0)?;

      if let Err(e) = state
        .redis
        .set_json(cache_key, &location, Duration::minutes(30))
        .await
      {
        error!("failed to put latest location in redis: {e:?}");
      }

      Ok(Some(location))
    }
  }
}

#[instrument]
pub async fn latest_location(State(mut state): State<AppState>) -> Response {
  match get_latest_location(&mut state).await {
    Ok(None) => ok::<()>("no_data", None),
    Ok(Some(location)) => ok("success", Some(location)),
    Err(e) => {
      error!("failed to query latest location: {e:?}");
      error("failed to query latest location")
    }
  }
}
