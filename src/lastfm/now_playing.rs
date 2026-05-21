use axum::{extract::State, response::Response};
use serde::{Deserialize, Serialize};
use worker::{KvStore, console_error};

use crate::{AppState, lastfm::build_request, resp::{error, ok}};

const CACHE_KEY: &str = "lastfm_now_playing";

#[derive(Debug, thiserror::Error)]
enum NowPlayingError {
  #[error("no recent tracks")]
  None,

  #[error("reqwest error: {0}")]
  Reqwest(#[from] reqwest::Error)
}

#[worker::send]
pub async fn handle(State(state): State<AppState>) -> Result<Response, Response> {
  let now_playing = match state.kv.get(CACHE_KEY).json::<TrackNode>().await {
    Ok(Some(now_playing)) => Some(now_playing),
    Ok(None) => match fetch_now_playing(&state.lastfm_key, &state.kv).await {
      Ok(now_playing) => Some(now_playing),
      Err(e) => match e {
        NowPlayingError::None => None,
        e => return Err(error(&format!("error while fetching now_playing state: {e:?}")))
      }
    },
    Err(_e) => return Err(error("error while getting from seer_cache"))
  };

  Ok(ok("success", now_playing))
}

#[derive(Deserialize)]
struct ApiResponse {
  #[serde(rename = "recenttracks")]
  recent_tracks: RecentTracksNode
}

#[derive(Deserialize)]
struct RecentTracksNode {
  track: Vec<TrackNode>
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
  text: String
}

#[derive(Deserialize, Serialize, Clone)]
struct ImageNode {
  size: String,

  #[serde(rename = "#text")]
  text: String
}

async fn fetch_now_playing(key: &str, kv: &KvStore) -> Result<TrackNode, NowPlayingError> {
  let res = build_request(key, "user.getRecentTracks")
    .query(&[("limit", 1)])
    .send()
    .await?
    .json::<ApiResponse>()
    .await?;

  let tracks = res.recent_tracks.track;
  if tracks.len() == 0 {
    return Err(NowPlayingError::None)
  }

  let track = tracks[0].clone();
  
  if let Ok(opts) = kv.put(CACHE_KEY, &track) {
    let ttl = chrono::Duration::minutes(1).num_seconds() as u64;
    if let Err(e) = opts.expiration_ttl(ttl).execute().await {
      console_error!("kv put failed: {e}");
    }
  }

  Ok(track)
}