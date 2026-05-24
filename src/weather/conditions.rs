use axum::{extract::State, response::Response};
use serde::{Deserialize, Serialize};
use worker::{KvStore, console_error};

use crate::{
  geo::{self, util::Coords},
  resp::{error, ok},
  weather, AppState,
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
  Reqwest(#[from] reqwest::Error)
}

pub async fn get_conditions(key: &str, cache_key: &str, kv: &KvStore, coords: Coords) -> Result<WeatherResp, WeatherError> {
  let conditions = reqwest::Client::new()
    .get(weather::OPENWEATHER_BASE_URL)
    .query(&[
      ("appid", key),
      ("lat", &format!("{}", coords.latitude)),
      ("lon", &format!("{}", coords.longitude)),
    ])
    .send()
    .await?
    .json::<WeatherResp>()
    .await?;

  if let Ok(opts) = kv.put(cache_key, &conditions) {
    let ttl = chrono::Duration::hours(1).num_seconds() as u64;
    if let Err(e) = opts.expiration_ttl(ttl).execute().await {
      console_error!("kv put failed: {e:?}");
    }
  }

  Ok(conditions)
}

#[worker::send]
pub async fn handle(State(state): State<AppState>) -> Response {
  let Ok(kv) = state.cf.kv(crate::CACHE_NS) else {
    return error("failed to get kv");
  };

  let cache_key = "latest_weather";

  if let Ok(Some(weather)) = kv.get(cache_key).json::<WeatherResp>().await {
    return ok("success", Some(weather))
  }

  let Ok(d1) = state.cf.d1(crate::GEO_DB) else {
    return error("failed to get d1");
  };

  let location = geo::query::latest::get_latest_location(&kv, &d1).await;
  if location.is_err() {
    console_error!("failed to get latest location: {location:?}");
    return error("could not get latest location");
  }

  let location = location.unwrap();
  if location.is_none() {
    return error("no location available for weather query");
  }

  let conditions = match geo::util::extract_coords(location.unwrap()) {
    None => return error("unable to extract coords from the latest location"),
    Some(coords) => get_conditions(&state.openweather_key, cache_key, &kv, coords).await,
  };

  if conditions.is_err() {
    console_error!("current condition fetch failed: {:?}", conditions.unwrap_err());
    return error("failed to fetch current conditions")
  }

  let conditions = conditions.unwrap();


  ok("success", Some(conditions))
}
