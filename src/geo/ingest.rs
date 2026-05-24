use axum::{
  Json, extract::State, response::{IntoResponse, Response}
};
use geojson::GeoJson;
use serde::{Deserialize, Serialize};
use serde_json::json;
use worker::{console_error, console_log, wasm_bindgen::JsValue};

use crate::{resp::error, AppState};

#[derive(Debug, Serialize, Deserialize)]
pub struct Trip {
  distance: u32,
  mode: String,
  current_location: GeoJson,
  start_location: GeoJson,
  start: chrono::DateTime<chrono::Local>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngestionPayload {
  locations: Vec<GeoJson>,
  current: Option<GeoJson>,
  trip: Option<Trip>,
}

#[worker::send]
pub async fn handle(
  State(state): State<AppState>,
  Json(payload): Json<IngestionPayload>,
) -> Response {
  let Ok(d1) = state.cf.d1(crate::GEO_DB) else {
    return error("failed to get d1");
  };

  let mut stmts = vec![];
  for location in &payload.locations {
    let stmt = d1.prepare("insert into record(data) values (?);");

    let bound = stmt.bind(&[JsValue::from_str(&location.to_string())]);
    if bound.is_err() {
      console_error!("failed to bind statement for location entry {:?}", location);
      continue;
    }

    stmts.push(bound.unwrap());
  }

  console_log!("prepared {} location updates...", stmts.len());
  match d1.batch(stmts).await {
    Ok(r) => {
      console_log!("saved {} location updates!", r.len());
      (axum::http::StatusCode::OK, Json(json!({"result": "ok"}))).into_response()
    },
    Err(e) => error(&format!("d1 batch submission failure: {e:?}"))
  }
}
