pub mod now_playing;

const API_BASE_URL: &str = "http://ws.audioscrobbler.com/2.0/";
pub const SECRET_KEY: &str = "LASTFM_KEY";

fn build_request(key: &str, method: &str) -> reqwest::RequestBuilder {
  reqwest::Client::new()
    .get(API_BASE_URL)
    .query(&[
      ("method", method),
      ("user", "jacobandersen_"),
      ("api_key", key), 
      ("format", "json")
    ])
}