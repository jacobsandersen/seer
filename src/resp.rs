use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;

#[derive(Serialize)]
struct Resp<'a, T> where T: Serialize {
  message: &'a str,
  data: Option<T>
}

pub fn ok<T>(message: &str, data: Option<T>) -> Response
where T: Serialize {
  to_resp(StatusCode::OK, message, data)
}

pub fn error(message: &str) -> Response {
  to_resp::<()>(StatusCode::INTERNAL_SERVER_ERROR, message, None)
}

fn to_resp<T>(status: StatusCode, message: &str, data: Option<T>) -> Response 
where T: Serialize {
  (status, Json(Resp { message, data })).into_response()
}