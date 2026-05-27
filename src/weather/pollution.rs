use axum::{extract::State, response::Response};
use tracing::instrument;

use crate::{resp::ok, AppState};

#[instrument(skip(_state))]
pub async fn current_pollution(State(_state): State<AppState>) -> Response {
  ok::<()>("not_yet_implemented", None)
}
