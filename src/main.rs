#![allow(non_upper_case_globals)]
#[macro_use]
extern crate lazy_static;

extern crate rustc_serialize;
use rustc_serialize::json;

extern crate rs_es;
use rs_es::Client;

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
                                                      .unwrap_or("config.toml".to_owned()));
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

  let params = req.get_ref::<Params>().ok().unwrap();
  let result = es.search_query()
                 .with_indexes(&config.es.indexes.clone()
                                                 .iter()
                                                 .map(|e| &**e)
                                                 .collect::<Vec<&str>>())
                 .with_query(&User::search_filters(params))
                 .with_sort(&User::sorting_criteria())
                 .send()
                 .ok()
                 .unwrap();

  let users = result.hits.hits.into_iter()
                              .map(|hit| {
                                let talent: TalentsSearchResult = hit.source().unwrap();
                                talent.id
                              })
                              .collect::<Vec<i32>>();

  let content_type = "application/json".parse::<Mime>().unwrap();
  Ok(Response::with((content_type, status::Ok, json::encode(&users).unwrap())))
}
