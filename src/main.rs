#![allow(non_upper_case_globals)]
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

extern crate searchspot;
use searchspot::config::*;
use searchspot::search::SearchResult;

#[macro_use] pub mod macros;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;

extern crate chrono;
mod resources;
use resources::user::Talent;

use std::collections::HashMap;
use std::io::Read;
use std::env;

lazy_static! {
  static ref config: Config = match env::args().nth(1) {
    Some(file) => Config::from_file(file),
    None       => Config::from_env()
  };
}

fn main() {
  let host = format!("{}:{}", config.http.host, config.http.port);

  println!("Searchspot v{}\n{}\n{}\n", env!("CARGO_PKG_VERSION"), config.es, config.http);

  let mut router = Router::new();
  handle_talents_search(&mut router);
  handle_talents_indexing(&mut router);
  handle_talents_reset(&mut router);

  let mut chain = Chain::new(router);

  // for some reasons, this link makes heroku crash
  if env::var("DYNO").is_err() {
    chain.link(Logger::new(None));
  }

  Iron::new(chain).http(&*host).unwrap();
}

fn handle_talents_search(mut router: &mut Router) {
  router.get("/talents", move |r: &mut Request| {
    let mut es = Client::new(&*config.es.host, config.es.port);
    search_talents(r, &mut es, &*config.es.index)
  });
}

fn handle_talents_indexing(mut router: &mut Router) {
  router.post("/talents", move |r: &mut Request| {
    let mut es = Client::new(&*config.es.host, config.es.port);
    index_talents(r, &mut es, &*config.es.index)
  });
}

fn handle_talents_reset(mut router: &mut Router) {
  router.delete("/talents", move |r: &mut Request| {
    let mut es = Client::new(&*config.es.host, config.es.port);
    reset_talents(r, &mut es, &*config.es.index)
  });
}

macro_rules! try_or_422 {
  ($expr:expr) => (match $expr {
    Ok(val)  => val,
    Err(err) => {
      let content_type = "application/json".parse::<Mime>().unwrap();
      let mut error = HashMap::new();
      error.insert("error", format!("{}", err));

      return Ok(Response::with(
        (content_type, status::UnprocessableEntity, json::encode(&error).unwrap())
      ))
    }
  })
}

fn search_talents(req: &mut Request, mut es: &mut Client, index: &str) -> IronResult<Response> {
  let params   = try_or_422!(req.get_ref::<Params>());
  let response = SearchResult {
    results: Talent::search(&mut es, index, params),
    params:  params.clone()
  };

  let content_type = "application/json".parse::<Mime>().unwrap();
  Ok(Response::with(
    (content_type, status::Ok, try_or_422!(json::encode(&response.to_json())))
  ))
}

fn index_talents(req: &mut Request, mut es: &mut Client, index: &str) -> IronResult<Response> {
  let mut payload = String::new();
  req.body.read_to_string(&mut payload).unwrap();

  let talents: Vec<Talent> = try_or_422!(json::decode(&payload));
  for talent in talents {
    try_or_422!(talent.index(es, index));
  }

  Ok(Response::with(status::Created))
}

fn reset_talents(_: &mut Request, mut es: &mut Client, index: &str) -> IronResult<Response> {
  match Talent::reset_index(&mut es, index) {
    Ok(_)  => Ok(Response::with(status::NoContent)),
    Err(_) => Ok(Response::with(status::UnprocessableEntity))
  }
}
