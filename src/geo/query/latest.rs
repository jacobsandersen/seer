use axum::{extract::State, response::Response};
use geojson::GeoJson;
use worker::{D1Database, KvStore, console_error};

use crate::{AppState, resp::{error, ok}};


#[derive(Debug, thiserror::Error)]
pub enum LatestLocationError {
  #[error("worker error occurred: {0}")]
  Worker(#[from] worker::Error),

  #[error("serde error occurred: {0}")]
  Serde(#[from] serde_json::Error)
}

pub async fn get_latest_location(kv: &KvStore, d1: &D1Database) -> Result<Option<GeoJson>, LatestLocationError> {
  let cache_key = "geo_latest_pos";

  if let Ok(Some(pos)) = kv.get(cache_key).json::<GeoJson>().await {
    return Ok(Some(pos));
  }

  let stmt = d1.prepare("select data from record order by timestamp desc limit 1");
  let result = stmt.first::<String>(Some("data")).await;
  if result.is_err() {
    return Err(LatestLocationError::Worker(result.unwrap_err()));
  }

  match result.unwrap() {
    None => Ok(None),
    Some(location) => {
      let location = serde_json::from_str::<GeoJson>(&location);
      if location.is_err() {
        return Err(LatestLocationError::Serde(location.unwrap_err()));
      }

      let location = location.unwrap();

      if let Ok(opts) = kv.put(cache_key, &location) {
        let ttl = chrono::Duration::minutes(30).num_seconds() as u64;
        if let Err(e) = opts.expiration_ttl(ttl).execute().await {
          console_error!("kv put failed: {e:?}");
        }
      }

      Ok(Some(location))
    }
  }
}

#[worker::send]
pub async fn handle(State(state): State<AppState>) -> Response {
  let Ok(kv) = state.cf.kv(crate::CACHE_NS) else {
    return error("failed to get kv")
  };

  let Ok(d1) = state.cf.d1(crate::GEO_DB) else {
    return error("failed to get d1")
  };

  match get_latest_location(&kv, &d1).await {
    Ok(None) => ok::<()>("no_data", None),
    Ok(Some(location)) => ok("success", Some(location)),
    Err(e) => {
      console_error!("failed to query latest location: {e:?}");
      error("failed to query latest location")
    }
  }
}
