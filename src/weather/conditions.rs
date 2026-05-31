use axum::{extract::State, response::Response};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use tracing::{error, instrument};

use crate::{
  geo::{self, util::Coords},
  redis::JsonExt,
  resp::{error, ok},
  weather::build_url,
  AppState,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct WeatherResp {
  name: String,
  main: WeatherMainNode,
  wind: WeatherWindNode,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WeatherMainNode {
  temp: f64,
  feels_like: f64,
  humidity: u16,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WeatherWindNode {
  speed: f64,
  deg: u16,
  gust: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum WeatherError {
  #[error("reqwest error: {0}")]
  Reqwest(#[from] reqwest::Error),
}

#[instrument(skip(state))]
pub async fn get_conditions(
  state: &mut AppState,
  cache_key: &str,
  coords: Coords,
) -> Result<WeatherResp, WeatherError> {
  let conditions = reqwest::Client::new()
    .get(build_url("weather"))
    .query(&[
      ("appid", &state.config.openweather_key),
      ("lat", &format!("{}", coords.latitude)),
      ("lon", &format!("{}", coords.longitude)),
    ])
    .send()
    .await?
    .json::<WeatherResp>()
    .await?;

  if let Err(e) = state
    .redis
    .set_json(cache_key, &conditions, Duration::hours(1))
    .await
  {
    error!("failed to put latest weather conditions in redis: {e:?}");
  }

  Ok(conditions)
}

#[instrument(skip(state))]
pub async fn current_conditions(State(mut state): State<AppState>) -> Result<Response, Response> {
  let cache_key = "seer:weather:conditions";

  if let Ok(Some(weather)) = state.redis.get_json::<WeatherResp>(cache_key).await {
    return Ok(ok("success", Some(weather)));
  }

  let location = geo::query::latest::get_latest_location(&mut state).await;
  if location.is_err() {
    error!("failed to get latest location: {location:?}");
    return Err(error("could not get latest location"));
  }

  let location = location.unwrap();
  if location.is_none() {
    return Err(error("no location available for weather query"));
  }

  let conditions = match geo::util::extract_coords(location.unwrap()) {
    None => return Err(error("unable to extract coords from the latest location")),
    Some(coords) => get_conditions(&mut state, cache_key, coords).await,
  }
  .map_err(|e| {
    error!("weather query error: {e:?}");
    error("failed to query weather")
  })?;

  Ok(ok("success", Some(conditions)))
}
