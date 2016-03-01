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

lazy_static! {
  static ref config: Config = match env::args().nth(1) {
    Some(file) => Config::from_file(file),
    None       => Config::from_env()
  };
}

fn main() {
  let host = format!("{}:{}", config.http.host, config.http.port);

  println!("Honeysearch v{}\n{}\n{}\n", env!("CARGO_PKG_VERSION"), config.es, config.http);

  let mut router = Router::new();
  router.get("/talents", talents);

  let mut chain = Chain::new(router);
  chain.link(Logger::new(None));
  Iron::new(chain).http(&*host).unwrap();
}

fn talents(req: &mut Request) -> IronResult<Response> {
  let mut es = Client::new(&*config.es.host, config.es.port);

  let params  = req.get_ref::<Params>().ok().unwrap();
  let indexes = config.es.indexes.iter()
                          .map(|e| &**e)
                          .collect::<Vec<&str>>();

  let response = SearchResult {
    results: Talent::search(&mut es, indexes, params),
    params:  params.clone()
  };

  let content_type = "application/json".parse::<Mime>().unwrap();
  Ok(Response::with((content_type, status::Ok, json::encode(&response.to_json()).unwrap())))
}
