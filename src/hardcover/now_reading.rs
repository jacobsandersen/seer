use axum::{extract::State, response::Response};
use serde::{Deserialize, Serialize};
use graphql_client::GraphQLQuery;
use worker::{KvStore, console_error};

use crate::{AppState, hardcover::HARDCOVER_GRAPHQL_ENDPOINT, resp::{error, ok}};
const CACHE_KEY: &str = "hardcover_now_reading";

#[worker::send]
pub async fn handle(State(state): State<AppState>) -> Result<Response, Response> {
  let now_reading = match state.kv.get(CACHE_KEY).json::<NowReading>().await {
    Ok(Some(now_reading)) => Some(now_reading),
    Ok(None) => match fetch_now_reading(&state.hardcover_key, &state.kv).await {
      Ok(now_reading) => Some(now_reading),
      Err(e) => match e {
        NowReadingError::None => None,
        e => {
          return Err(error(&format!("error while fetching now_reading state: {e:?}")))
        }
      }
    },
    Err(_e) => return Err(error("error while getting from seer_cache"))
  };

  Ok(ok("success", now_reading))
}

#[derive(GraphQLQuery)]
#[graphql(
  schema_path = "graphql/hardcover/schema.json",
  query_path = "graphql/hardcover/query/currently_reading.graphql"
)]
pub struct CurrentlyReadingQuery;

#[derive(Serialize, Deserialize)]
struct NowReading {
  title: String,
  author: String,
  image: String
}

#[derive(Deserialize)]
struct ApiResponse {
  data: ResponseData
}

#[derive(Deserialize)]
struct ResponseData {
  me: Vec<MeNode>
}

#[derive(Deserialize)]
struct MeNode {
  user_books: Vec<UserBookNode>
}

#[derive(Deserialize)]
struct UserBookNode {
  book: BookNode
}

#[derive(Deserialize)]
struct BookNode {
  title: String,
  contributions: Vec<ContributionNode>,
  image: ImageNode
}

#[derive(Deserialize)]
struct ContributionNode {
  author: AuthorNode
}

#[derive(Deserialize)]
struct AuthorNode {
  name: String
}

#[derive(Deserialize)]
struct ImageNode {
  url: String
}

impl TryInto<NowReading> for ApiResponse {
  type Error = NowReadingError;

  fn try_into(self) -> Result<NowReading, Self::Error> {
    let me = self.data.me;
    if me.len() == 0 {
      return Err(NowReadingError::MissingMe)
    }

    let books = &me[0].user_books;
    if books.len() == 0 {
      return Err(NowReadingError::None)
    }

    let book = &books[0].book;
    let mut author = "Unknown";
    let contributions = &book.contributions;
    if contributions.len() > 0 {
      let author_contributions = &contributions[0];
      author = &author_contributions.author.name;
    }

    Ok(NowReading { title: book.title.clone(), author: author.to_string(), image: book.image.url.clone() })
  }
}

#[derive(Debug, thiserror::Error)]
enum NowReadingError {
  #[error("no `me` entries in hardcover response")]
  MissingMe,

  #[error("not reading any books")]
  None,


  #[error("reqwest error: {0}")]
  Reqwest(#[from] reqwest::Error)
}

async fn fetch_now_reading(key: &str, kv: &KvStore) -> Result<NowReading, NowReadingError> {
  let query = CurrentlyReadingQuery::build_query(currently_reading_query::Variables {});

  let res = reqwest::Client::new()
    .post(HARDCOVER_GRAPHQL_ENDPOINT)
    .header("Authorization", format!("Bearer {key}"))
    .json(&query)
    .send()
    .await?
    .json::<ApiResponse>()
    .await?
    .try_into()?;

  if let Ok(opts) = kv.put(CACHE_KEY, &res) {
    let ttl = chrono::Duration::days(3).num_seconds() as u64;
    if let Err(e) = opts.expiration_ttl(ttl).execute().await {
      console_error!("kv put failed: {e}");
    }
  }

  Ok(res)
}