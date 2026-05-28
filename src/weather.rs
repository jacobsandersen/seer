pub mod conditions;
pub mod pollution;

pub const SECRET_KEY: &str = "OPENWEATHER_KEY";
const OPENWEATHER_BASE_URL: &str = "https://api.openweathermap.org/data/2.5/";

fn build_url(action: &str) -> String {
  return format!("{}/{}?units=metric", OPENWEATHER_BASE_URL, action)
}
