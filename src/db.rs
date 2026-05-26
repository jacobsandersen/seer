use std::path::Path;

use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool, Result};
use tracing::info;

use crate::config::Db;

pub async fn initialize_db(cfg: &Db) -> Result<PgPool> {
  info!("connecting to database...");
  let url = format!(
    "postgres://{}:{}@{}:{}/{}",
    urlencoding::encode(&cfg.username),
    urlencoding::encode(&cfg.password),
    cfg.host,
    cfg.port,
    cfg.database
  );

  let pool = PgPoolOptions::new()
    .max_connections(10)
    .connect(&url)
    .await?;

  info!("running migrations...");
  let migrator = Migrator::new(Path::new("./migrations")).await?;
  migrator.run(&pool).await?;

  Ok(pool)
}
