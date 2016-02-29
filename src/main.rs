#![allow(non_upper_case_globals)]
#[macro_use]
extern crate lazy_static;

extern crate rustc_serialize;
use rustc_serialize::json::{self, ToJson};

extern crate rs_es;
use rs_es::Client;

extern crate iron;
use iron::prelude::*;
use iron::status;
use iron::mime::Mime;

extern crate logger;
use logger::Logger;

extern crate router;
use router::Router;

extern crate params;
use params::*;

extern crate honeysearch;
use honeysearch::resources::user::Talent;
use honeysearch::config::*;
use honeysearch::search::SearchResult;

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

  let mut chain = Chain::new(router);
  chain.link(Logger::new(None));
  Iron::new(chain).http(&*host).unwrap();
}

fn talents(req: &mut Request) -> IronResult<Response> {
  let es = Client::new(&*config.es.host, config.es.port);

  let params  = req.get_ref::<Params>().ok().unwrap();
  let indexes = config.es.indexes.iter()
                                 .map(|e| &**e)
                                 .collect::<Vec<&str>>();

  let response = SearchResult {
    results: Talent::search(es, params, &indexes),
    params:  params.clone()
  };

  let content_type = "application/json".parse::<Mime>().unwrap();
  Ok(Response::with((content_type, status::Ok, json::encode(&response.to_json()).unwrap())))
}
