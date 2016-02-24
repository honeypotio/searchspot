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

extern crate hyper;
use hyper::Server;
use hyper::server::Request;
use hyper::server::Response;
use hyper::header::ContentType;
use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};

mod user;
use user::User;

mod company;

mod filters;
use filters::{visibility_filters, VectorOfTerms};

#[derive(Debug, RustcDecodable)]
struct TalentsSearchResult {
  id: i32
}

const PG_URL: &'static str = "postgres://lando@localhost/lando_development";
const ES_INDEXES: &'static [&'static str] = &["honeypot_dev_talents"];

fn main() {
  println!("Listening on http://127.0.0.1:3000");
  Server::http("127.0.0.1:3000").unwrap().handle(listener).unwrap();
}

fn listener(_: Request, mut res: Response) {
  res.headers_mut().set(ContentType(Mime(
                          TopLevel::Application,
                          SubLevel::Json,
                          vec![
                            (Attr::Charset, Value::Utf8)
                          ]
                        )));

  let mut es = Client::new("localhost", 9200);
  let     pg = Connection::connect(PG_URL, SslMode::None).unwrap();

  let roles:         Vec<&str>   = vec!["Frontend", "Backend"];
  let languages:     Vec<&str>   = vec![];
  let experience:    Vec<&str>   = vec![];
  let locations:     Vec<&str>   = vec![];
  let authorization: Vec<&str>   = vec![];
  let company_ids:   Vec<i32>    = vec![];
  let company_id:    Option<i32> = None;

  let query = Query::build_filtered(Filter::build_bool()
                                           .with_must(
                                             vec![
                                               <Filter as VectorOfTerms<&str>>::build_terms(
                                                 "work_roles", &roles),

                                               <Filter as VectorOfTerms<&str>>::build_terms(
                                                 "work_languages", &languages),

                                               <Filter as VectorOfTerms<&str>>::build_terms(
                                                 "work_experience", &experience),

                                               <Filter as VectorOfTerms<&str>>::build_terms(
                                                 "work_locations", &locations),

                                               <Filter as VectorOfTerms<&str>>::build_terms(
                                                "work_authorization", &authorization),

                                               visibility_filters(&pg, &company_id)
                                             ].into_iter()
                                              .flat_map(|x| x)
                                              .collect::<Vec<Filter>>())
                                           .with_must_not(
                                             vec![
                                               <Filter as VectorOfTerms<i32>>::build_terms(
                                                 "company_ids", &company_ids),

                                               <Filter as VectorOfTerms<i32>>::build_terms(
                                                 "blocked_companies", &company_ids)
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

  res.send(&json::encode(&user_ids).unwrap().into_bytes()).unwrap();
}
