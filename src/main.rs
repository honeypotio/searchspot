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
use iron::prelude::*;
use iron::status;
use iron::mime::Mime;

extern crate router;
use router::Router;

extern crate params;
use params::*;

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

macro_rules! vec_from_params {
  ($params:expr, $param:expr) => {
    match $params.find(&[$param]) {
      Some(val) => Vec::from_value(val)
                       .unwrap_or(vec![]),
      None => vec![],
    }
  }
}

fn talents(req: &mut Request) -> IronResult<Response> {
  let mut es = Client::new("localhost", 9200);
  let     pg = Connection::connect(PG_URL, SslMode::None).unwrap();

  let params = req.get_ref::<Params>().ok().unwrap();

  let company_id = match params.find(&["company_id"]) {
    Some(company_id) => i32::from_value(company_id)
                            .map(|id| vec![id])
                            .unwrap_or(vec![]),
    None => vec![]
  };

  let query = Query::build_filtered(Filter::build_bool()
                                           .with_must(
                                             vec![
                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                 "work_roles", &vec_from_params!(params, "work_roles")),

                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                 "work_languages", &vec_from_params!(params, "work_languages")),

                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                 "work_experience", &vec_from_params!(params, "work_experience")),

                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                 "work_locations", &vec_from_params!(params, "work_locations")),

                                               <Filter as VectorOfTerms<String>>::build_terms(
                                                "work_authorization", &vec_from_params!(params, "work_authorization")),

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

#[cfg(test)]
mod tests {
  use params::*;

  #[test]
  fn test_vec_from_params() {
    {
      let mut params = Map::new();
      params.assign("work_roles[]", Value::String("Fullstack".into())).unwrap();
      params.assign("work_roles[]", Value::String("DevOps".into())).unwrap();

      let work_roles: Vec<String> = vec_from_params!(params, "work_roles");
      assert_eq!(work_roles, vec!["Fullstack", "DevOps"]);
    }

    {
      let mut params = Map::new();
      params.assign("work_roles[]", Value::String("".into())).unwrap();

      let work_roles: Vec<String> = vec_from_params!(params, "work_roles");
      assert_eq!(work_roles, vec![""]); // vec![]?
    }

    {
      let work_roles: Vec<String> = vec_from_params!(Map::new(), "work_roles");
      assert_eq!(work_roles, Vec::<String>::new());
    }
  }

  macro_rules! i32_vec_from_params {
    ($params:expr, $param:expr) => {
      match $params.find(&[$param]) {
        Some(company_id) => i32::from_value(company_id)
                                .map(|id| vec![id])
                                .unwrap_or(vec![]),
        None => vec![]
      }
    }
  }

  #[test]
  fn test_company_id() {
    {
      let mut params = Map::new();
      params.assign("company_id", Value::String("4".into())).unwrap();

      let company_id: Vec<i32> = i32_vec_from_params!(params, "company_id");
      assert_eq!(company_id, vec![4]);
    }

    {
      let mut params = Map::new();
      params.assign("company_id", Value::String("".into())).unwrap();

      let company_id: Vec<i32> = i32_vec_from_params!(params, "company_id");
      assert_eq!(company_id, vec![]);
    }

    {
      let mut params = Map::new();
      params.assign("company_id", Value::String("madukapls".into())).unwrap();

      let company_id: Vec<i32> = i32_vec_from_params!(params, "company_id");
      assert_eq!(company_id, vec![]);
    }

    {
      let mut params = Map::new();
      params.assign("company_id[]", Value::String("madukapls".into())).unwrap();

      let company_id: Vec<i32> = i32_vec_from_params!(params, "company_id");
      assert_eq!(company_id, vec![]);
    }

    {
      let company_id: Vec<i32> = i32_vec_from_params!(Map::new(), "company_id");
      assert_eq!(company_id, vec![]);
    }
  }
}
