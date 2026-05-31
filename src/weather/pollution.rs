use axum::{extract::State, response::Response};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use tracing::{error, instrument};

use crate::{
  geo::{self, util::Coords},
  redis::JsonExt,
  resp::{error, not_found, ok},
  weather::build_url,
  AppState,
};

#[derive(Debug, Deserialize)]
struct PollutionResp {
  list: Vec<PollutionData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PollutionData {
  dt: u64,
  main: PollutionMain,
  components: PollutionComponents,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PollutionMain {
  aqi: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PollutionComponents {
  co: f64,
  no: f64,
  no2: f64,
  o3: f64,
  so2: f64,
  pm2_5: f64,
  pm10: f64,
  nh3: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum PollutionError {
  #[error("reqwest error: {0}")]
  Reqwest(#[from] reqwest::Error),
}

#[instrument(skip(state))]
pub async fn get_pollution(
  state: &mut AppState,
  cache_key: &str,
  coords: Coords,
) -> Result<Option<PollutionData>, PollutionError> {
  let pollution = reqwest::Client::new()
    .get(build_url("air_pollution"))
    .query(&[
      ("appid", &state.config.openweather_key),
      ("lat", &format!("{}", coords.latitude)),
      ("lon", &format!("{}", coords.longitude)),
    ])
    .send()
    .await?
    .json::<PollutionResp>()
    .await?;

  if let Some(data) = pollution.list.get(0) {
    if let Err(e) = state
      .redis
      .set_json(cache_key, data, Duration::hours(1))
      .await
    {
      error!("failed to put latest weather conditions in redis: {e:?}");
    }

    return Ok(Some(data.clone()));
  }

  return Ok(None);
}

#[instrument(skip(state))]
pub async fn current_pollution(State(mut state): State<AppState>) -> Result<Response, Response> {
  let cache_key = "seer:weather:pollution";

  if let Ok(Some(pollution)) = state.redis.get_json::<PollutionData>(cache_key).await {
    return Ok(ok("success", Some(pollution)));
  }

  let location = geo::query::latest::get_latest_location(&mut state).await;
  if location.is_err() {
    error!("failed to get latest location: {location:?}");
    return Err(error("could not get latest location"));
  }

  let location = location.unwrap();
  if location.is_none() {
    return Err(error("no location available for pollution query"));
  }

  let pollution = match geo::util::extract_coords(location.unwrap()) {
    None => return Err(error("unable to extract coords from the latest location")),
    Some(coords) => get_pollution(&mut state, cache_key, coords).await,
  }
  .map_err(|e| {
    error!("pollution query failed: {e:?}");
    error("failed to fetch current pollution")
  })?
  .ok_or_else(|| not_found("pollution data not found"))?;

  Ok(ok("success", Some(pollution)))
}
