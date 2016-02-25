extern crate chrono;

extern crate rustc_serialize;
use rustc_serialize::json;

extern crate rs_es;
use rs_es::Client;
use rs_es::operations::search::{Sort, SortField, Order};
use rs_es::query::{Filter, Query};

extern crate postgres;
extern crate postgres_array;
use postgres::{Connection, SslMode};

extern crate iron;
extern crate router;
extern crate urlencoded;
use iron::prelude::*;
use iron::status;
use iron::mime::Mime;
use router::Router;
use urlencoded::UrlEncodedQuery;

mod user;
use user::User;

mod company;

mod filters;
use filters::{visibility_filters, VectorOfTerms};

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

  let params = req.get_ref::<UrlEncodedQuery>().ok().unwrap();
  let empty_vec: Vec<String> = vec![];

  let company_id = match params.clone().get_mut("company_id") {
    Some(company_id) => company_id[0].parse::<i32>()
                                     .map(|x| vec![x])
                                     .unwrap_or(vec![]),
    None => vec![]
  };

  let query = Query::build_filtered(Filter::build_bool()
                                           .with_must(
                                             vec![
                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                 "work_roles", params.get("work_roles")
                                                                     .unwrap_or(&empty_vec)),

                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                 "work_languages", params.get("work_languages")
                                                                         .unwrap_or(&empty_vec)),

                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                 "work_experience", params.get("work_experience")
                                                                          .unwrap_or(&empty_vec)),

                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                 "work_locations", params.get("work_locations")
                                                                         .unwrap_or(&empty_vec)),

                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                "work_authorization", params.get("work_authorization")
                                                                            .unwrap_or(&empty_vec)),

                                               visibility_filters(&pg, match company_id.is_empty() {
                                                 true  => None,
                                                 false => Some(company_id[0]),
                                               })
                                             ].into_iter()
                                              .flat_map(|x| x)
                                              .collect::<Vec<Filter>>())
                                           .with_must_not(
                                             vec![
                                               <Filter as VectorOfTerms<i32>>::build_terms(
                                                 "company_ids", &company_id),

                                               <Filter as VectorOfTerms<i32>>::build_terms(
                                                 "blocked_companies", &company_id)
                                             ].into_iter()
                                              .flat_map(|x| x)
                                              .collect::<Vec<Filter>>())
                                           .build())
                    .build();

  let result = es.search_query()
                 .with_indexes(ES_INDEXES)
                 .with_query(&query)
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
