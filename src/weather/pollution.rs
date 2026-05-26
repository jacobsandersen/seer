use axum::{extract::State, response::Response};
use tracing::instrument;

use crate::{resp::ok, AppState};

#[instrument]
pub async fn handle(State(_state): State<AppState>) -> Response {
  ok::<()>("not_yet_implemented", None)
}
