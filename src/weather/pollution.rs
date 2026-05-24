use axum::{extract::State, response::Response};

use crate::{resp::ok, AppState};

#[worker::send]
pub async fn handle(State(_state): State<AppState>) -> Response {
  ok::<()>("not_yet_implemented", None)
}
