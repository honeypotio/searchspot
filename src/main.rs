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

use std::env;

#[derive(Debug, RustcDecodable)]
struct TalentsSearchResult {
  id: i32
}

const PG_URL:     &'static str            = "postgres://lando@localhost/lando_development";
const ES_INDEXES: &'static [&'static str] = &["honeypot_dev_talents"];
const VERSION:    &'static str            = env!("CARGO_PKG_VERSION");

fn main() {
  let port = env::args().nth(1).unwrap_or(String::from("3000"));
  let host = format!("127.0.0.1:{}", port);

  println!("Honeysearch v{}", VERSION);
  println!("Listening on http://{}...", host);

  let mut router = Router::new();
  router.get("/talents", talents);

  Iron::new(router).http(&*host).unwrap();
}

fn talents(req: &mut Request) -> IronResult<Response> {
  let mut es = Client::new("localhost", 9200);
  let     pg = Connection::connect(PG_URL, SslMode::None).unwrap();

  let params = req.get_ref::<Params>().ok().unwrap();

  let result = es.search_query()
                 .with_indexes(ES_INDEXES)
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
