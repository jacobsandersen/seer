use std::time::Duration;

use redis::{
  aio::{ConnectionManager, ConnectionManagerConfig},
  AsyncCommands, Client, RedisError, RedisResult,
};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{info, info_span, instrument, Instrument};

use crate::config::Redis;

pub async fn initialize_redis(cfg: &Redis) -> Result<ConnectionManager, RedisError> {
  let url = format!("redis://{}:{}", &cfg.host, &cfg.port);

  info!("redis: connecting to: {url}");
  let client = Client::open(url)?;
  let mut conn = ConnectionManager::new_with_config(
    client,
    ConnectionManagerConfig::new()
      .set_connection_timeout(Some(Duration::from_secs(30)))
      .set_response_timeout(Some(Duration::from_secs(5))),
  )
  .await?;

  info!("redis: authenticating...");
  redis::cmd("AUTH")
    .arg(&cfg.username)
    .arg(&cfg.password)
    .query_async::<()>(&mut conn)
    .await?;

  info!("redis: selecting db...");
  redis::cmd("SELECT")
    .arg(&cfg.dbno)
    .query_async::<()>(&mut conn)
    .await?;

  info!("redis: ok");
  Ok(conn)
}

#[allow(async_fn_in_trait)]
pub trait JsonExt {
  async fn get_json<T: DeserializeOwned>(&mut self, key: &str) -> RedisResult<Option<T>>;
  async fn set_json<T: Serialize>(
    &mut self,
    key: &str,
    value: &T,
    ttl: chrono::Duration,
  ) -> RedisResult<()>;
}

impl JsonExt for ConnectionManager {
  #[instrument(fields(key = %key))]
  async fn get_json<T: DeserializeOwned>(&mut self, key: &str) -> RedisResult<Option<T>> {
    let raw: Option<String> = self.get(key).await?;
    match raw {
      Some(s) => Ok(Some(serde_json::from_str(&s).map_err(|e| {
        RedisError::from((redis::ErrorKind::Io, "json deserialize", e.to_string()))
      })?)),
      None => Ok(None),
    }
  }

  #[instrument(skip(value), fields(key = %key, ttl = %ttl))]
  async fn set_json<T: Serialize>(
    &mut self,
    key: &str,
    value: &T,
    ttl: chrono::Duration,
  ) -> RedisResult<()> {
    let json = serde_json::to_string(value)
      .map_err(|e| RedisError::from((redis::ErrorKind::Io, "json serialize", e.to_string())))?;

    self.set_ex(key, json, ttl.num_seconds() as u64).await
  }
}
