use crate::redis::JsonExt;
use axum::{extract::State, response::Response};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use tracing::{error, instrument};

use crate::{
  resp::{error, ok},
  AppState,
};

pub const SECRET_KEY: &str = "LASTFM_KEY";

const CACHE_KEY: &str = "lastfm_now_playing";
const API_BASE_URL: &str = "http://ws.audioscrobbler.com/2.0/";

#[derive(Debug, thiserror::Error)]
enum NowPlayingError {
  #[error("no recent tracks")]
  None,

  #[error("reqwest error: {0}")]
  Reqwest(#[from] reqwest::Error),
}

#[derive(Deserialize)]
struct ApiResponse {
  #[serde(rename = "recenttracks")]
  recent_tracks: RecentTracksNode,
}

#[derive(Deserialize)]
struct RecentTracksNode {
  track: Vec<TrackNode>,
}

#[derive(Deserialize, Serialize, Clone)]
struct TrackNode {
  name: String,
  url: String,
  artist: ArtistNode,
  image: Vec<ImageNode>,
}

#[derive(Deserialize, Serialize, Clone)]
struct ArtistNode {
  #[serde(rename = "#text")]
  text: String,
}

#[derive(Deserialize, Serialize, Clone)]
struct ImageNode {
  size: String,

  #[serde(rename = "#text")]
  text: String,
}

#[instrument]
pub async fn now_playing(State(mut state): State<AppState>) -> Response {
  if let Ok(Some(value)) = state.redis.get_json::<TrackNode>(CACHE_KEY).await {
    return ok("success", Some(value));
  }

  let now_playing = match fetch_now_playing(&mut state).await {
    Ok(now_playing) => Some(now_playing),
    Err(e) => match e {
      NowPlayingError::None => None,
      e => return error(&format!("error while fetching now_playing state: {e:?}")),
    },
  };

  ok("success", now_playing)
}

#[instrument]
async fn fetch_now_playing(state: &mut AppState) -> Result<TrackNode, NowPlayingError> {
  let res = reqwest::Client::new()
    .get(API_BASE_URL)
    .query(&[
      ("method", "user.getRecentTracks"),
      ("user", "jacobandersen_"),
      ("api_key", &state.config.lastfm_key),
      ("format", "json"),
      ("limit", "1"),
    ])
    .send()
    .await?
    .json::<ApiResponse>()
    .await?;

  let tracks = res.recent_tracks.track;
  if tracks.len() == 0 {
    return Err(NowPlayingError::None);
  }

  let track = tracks[0].clone();

  if let Err(e) = state
    .redis
    .set_json(CACHE_KEY, &track, Duration::minutes(1))
    .await
  {
    error!("failed to save lastfm data to redis: {e:?}");
  }

  Ok(track)
}
