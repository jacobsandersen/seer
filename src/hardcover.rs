use std::collections::HashMap;

use axum::{
  extract::{Query, State},
  response::Response,
};
use graphql_client::GraphQLQuery;
use serde::{Deserialize, Serialize};
use tracing::{error, instrument};

use crate::{
  hardcover::query::{my_books_query, MyBooksQuery},
  redis::JsonExt,
  resp::{error, ok},
  AppState,
};

pub const SECRET_KEY: &str = "HARDCOVER_KEY";

const HARDCOVER_GRAPHQL_ENDPOINT: &str = "https://api.hardcover.app/v1/graphql";

mod query {
  use chrono::NaiveDate;
  use graphql_client::GraphQLQuery;

  #[allow(non_camel_case_types)]
  pub type date = NaiveDate;

  #[derive(GraphQLQuery)]
  #[graphql(
    schema_path = "graphql/hardcover/schema.json",
    query_path = "graphql/hardcover/query/my_books.graphql"
  )]
  pub struct MyBooksQuery;
}

#[derive(Serialize, Deserialize)]
struct ResponsePagination {
  current_page: usize,
  total_pages: usize,
}

#[derive(Serialize, Deserialize)]
struct DomainBook {
  title: String,
  author: String,
  image: Option<String>,
  last_read: Option<chrono::NaiveDate>,
  times_read: u32,
}

#[derive(Serialize, Deserialize)]
struct BooksResponse {
  pagination: ResponsePagination,
  books: Vec<DomainBook>,
}

#[derive(Debug, thiserror::Error)]
enum BooksError {
  #[error("no data returned from hardcover")]
  NoData,

  #[error("no `me` entries in hardcover response")]
  MissingMe,

  #[error("no `aggregate` entry in hardcover response")]
  MissingAggregate,

  #[error("no such status: {0}")]
  UnknownStatus(String),

  #[error("reqwest error: {0}")]
  Reqwest(#[from] reqwest::Error),
}

#[derive(Debug)]
struct FetchContext<'a> {
  limit: usize,
  page: usize,
  status: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct RequestOpts {
  status: String,
  limit: usize,
  page: Option<usize>,
}

fn build_cache_key(status: &str, limit: usize, page: usize) -> String {
  format!("hardcover_{status}_{limit}_{page}")
}

fn status_from_str(s: &str) -> Option<usize> {
  let map = HashMap::from([
    ("wanted", 1),
    ("current", 2),
    ("read", 3),
    ("paused", 4),
    ("dnf", 5),
    ("ignored", 6),
  ]);

  map.get(s).copied()
}

#[instrument(skip(state))]
pub async fn books(State(mut state): State<AppState>, opts: Query<RequestOpts>) -> Response {
  let page = opts.page.unwrap_or(1).max(1);
  let cache_key = &build_cache_key(&opts.status, opts.limit, page);

  if let Ok(Some(value)) = state.redis.get_json::<BooksResponse>(cache_key).await {
    return ok("success", Some(value));
  }

  let res = fetch_books(
    &mut state,
    cache_key,
    &FetchContext {
      limit: opts.limit,
      page: page,
      status: &opts.status,
    },
  )
  .await;

  let books = match res {
    Ok(now_reading) => Some(now_reading),
    Err(e) => {
      return error(&format!(
        "error while fetching previously_read state: {e:?}"
      ))
    }
  };

  ok("success", books)
}

#[instrument]
async fn fetch_books<'a>(
  state: &mut AppState,
  cache_key: &str,
  ctx: &FetchContext<'a>,
) -> Result<BooksResponse, BooksError> {
  let status = status_from_str(&ctx.status);
  if status.is_none() {
    return Err(BooksError::UnknownStatus(ctx.status.to_string()));
  }

  let page = ctx.page;
  let limit = ctx.limit.max(1);
  let offset = (page - 1).max(0).checked_mul(limit).unwrap_or(0);

  let query = MyBooksQuery::build_query(my_books_query::Variables {
    status_id: status.map(|s| s as i64),
    limit: Some(limit as i64),
    offset: Some(offset as i64),
  });

  let res: (usize, Vec<DomainBook>) = reqwest::Client::new()
    .post(HARDCOVER_GRAPHQL_ENDPOINT)
    .header(
      "Authorization",
      format!("Bearer {}", state.config.hardcover_key),
    )
    .json(&query)
    .send()
    .await?
    .json::<graphql_client::Response<my_books_query::ResponseData>>()
    .await?
    .data
    .ok_or_else(|| BooksError::NoData)?
    .try_into()?;

  let res = BooksResponse {
    pagination: ResponsePagination {
      current_page: page,
      total_pages: (res.0 + limit - 1) / limit,
    },
    books: res.1,
  };

  if let Err(e) = state
    .redis
    .set_json(cache_key, &res, chrono::Duration::days(1))
    .await
  {
    error!("failed to put hardcover data in redis: {e:?}");
  }

  Ok(res)
}

impl TryInto<(usize, Vec<DomainBook>)> for my_books_query::ResponseData {
  type Error = BooksError;

  #[instrument(skip(self))]
  fn try_into(self) -> Result<(usize, Vec<DomainBook>), Self::Error> {
    if self.me.len() == 0 {
      return Err(BooksError::MissingMe);
    }

    let me = &self.me[0];

    let total_books = me
      .user_books_aggregate
      .aggregate
      .as_ref()
      .ok_or_else(|| BooksError::MissingAggregate)?
      .count as usize;

    let books = &me.user_books;
    if books.len() == 0 {
      return Ok((total_books, Vec::new()));
    }

    Ok((
      total_books,
      books
        .into_iter()
        .map(|user_book| {
          let book = &user_book.book;

          let authors: Vec<String> = book
            .contributions
            .iter()
            .filter_map(|c| c.author.as_ref())
            .map(|a| a.name.clone())
            .collect();

          let author = match authors.len() {
            0 => String::from("Unknown author"),
            1 => authors[0].clone(),
            n => format!("{} (and {} more)", authors[0], n),
          };

          let image = book.image.as_ref().and_then(|i| i.url.clone());

          DomainBook {
            title: book.title.clone().unwrap_or_default(),
            author: author,
            image: image,
            last_read: user_book.last_read_date,
            times_read: user_book.read_count as u32,
          }
        })
        .collect(),
    ))
  }
}
