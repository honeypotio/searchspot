#![allow(non_upper_case_globals)]
#[macro_use]
extern crate lazy_static;

extern crate rustc_serialize;
use rustc_serialize::json;

extern crate rs_es;
use rs_es::Client;
use rs_es::operations::search::{Sort, SortField, Order};

extern crate postgres;
use postgres::{Connection, SslMode};

extern crate iron;
use iron::prelude::*;
use iron::status;
use iron::mime::Mime;

extern crate router;
use router::Router;

extern crate params;
use params::*;

extern crate honeysearch;
use honeysearch::resources::user::User;
use honeysearch::config::*;

use std::env;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

lazy_static! {
  static ref config: Config = Config::load_config(env::args()
                                                      .nth(1)
                                                      .unwrap_or("honeysearch.toml".to_owned()));
}

fn main() {
  let host = format!("{}:{}", config.http.host, config.http.port);

  println!("Honeysearch v{}", VERSION);
  println!("Listening on http://{}...", host);

  let mut router = Router::new();
  router.get("/talents", talents);

  Iron::new(router).http(&*host).unwrap();
}

#[derive(Debug, RustcDecodable)]
struct TalentsSearchResult {
  id: i32
}

fn talents(req: &mut Request) -> IronResult<Response> {
  let mut es = Client::new(&*config.es.host, config.es.port);
  let     pg = Connection::connect(&*config.db.uri, SslMode::None).unwrap();

  let params = req.get_ref::<Params>().ok().unwrap();
  let result = es.search_query()
                 .with_indexes(&config.es.indexes.clone()
                                                 .iter()
                                                 .map(|e| &**e)
                                                 .collect::<Vec<&str>>())
                 .with_query(&User::search_filters(&pg, params))
                 .with_sort(&Sort::new(
                   vec![
                     SortField::new("updated_at", Some(Order::Desc)).build()
                   ]))
                 .send()
                 .ok()
                 .unwrap();

  let user_ids = result.hits.hits.into_iter()
                                 .filter_map(|hit| {
                                   let talent: TalentsSearchResult = hit.source().unwrap();
                                   User::find(&pg, &talent.id)
                                 })
                                 .collect::<Vec<User>>();

  let content_type = "application/json".parse::<Mime>().unwrap();
  Ok(Response::with((content_type, status::Ok, json::encode(&user_ids).unwrap())))
}
