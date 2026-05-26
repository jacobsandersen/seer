use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use validator::Validate;

static RE_IPV4: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"(\b25[0-5]|\b2[0-4][0-9]|\b[01]?[0-9][0-9]?)(\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)){3}",
  )
  .unwrap()
});

#[derive(Debug, Validate, Deserialize)]
pub struct SeerConfig {
  /// Server binding settings
  #[validate(nested)]
  pub binding: Binding,

  /// The hardcover API key
  pub hardcover_key: String,

  /// The lastfm API key
  pub lastfm_key: String,

  /// The openweathermap API key
  pub openweather_key: String,

  /// The fixed auth string that other services must specify in order
  /// to call Seer
  pub fixed_auth: String,

  /// The telemetry configuration, which enables reporting traces to OTel
  #[validate(nested)]
  pub telemetry: Telemetry,

  /// The redis configuration which enables caching
  #[validate(nested)]
  pub redis: Redis,

  /// The db configuration for data storage
  #[validate(nested)]
  pub db: Db,
}

#[derive(Debug, Validate, Deserialize)]
pub struct Binding {
  /// The IP to bind to
  #[validate(regex(path = *RE_IPV4))]
  pub ip: String,

  /// The port to bind to
  pub port: u16,
}

impl std::fmt::Display for Binding {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}:{}", self.ip, self.port)
  }
}

#[derive(Debug, Validate, Deserialize)]
pub struct Telemetry {
  /// Whether to use OTel
  pub enable: bool,

  /// Where to ship OTel junk
  #[validate(url)]
  pub otel_exporter_endpoint: String,
}

#[derive(Debug, Validate, Deserialize)]
pub struct Redis {
  /// The redis host
  pub host: String,

  /// The redis port
  pub port: u16,

  /// The redis username
  pub username: String,

  /// The redis password
  pub password: String,

  /// Which redis database to use (0-15)
  #[validate(range(min = 0, max = 15))]
  pub dbno: u8,
}

#[derive(Debug, Validate, Deserialize)]
pub struct Db {
  /// The database host
  pub host: String,

  /// The database port
  pub port: u16,

  /// The database username
  pub username: String,

  /// The database password
  pub password: String,

  /// The database name
  pub database: String,
}
