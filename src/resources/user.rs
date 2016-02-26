use chrono::UTC;
use chrono::datetime::DateTime;

use postgres::Connection;

use params::*;

use rs_es::query::{Filter, Query};
use rs_es::units::JsonVal;

use terms::VectorOfTerms;
use resources::company::Company;

pub struct User;

impl User {
  pub fn visibility_filters(conn: &Connection, company_id: Option<i32>) -> Vec<Filter> {
    let now = DateTime::timestamp(&UTC::now());

    let visibility_rules = Filter::build_bool()
                                  .with_must(
                                    vec![
                                      Filter::build_term("accepted", true)
                                             .build(),
                                      Filter::build_range("batch_starts_at")
                                             .with_lt(JsonVal::from(now))
                                             .with_format("epoch_second")
                                             .build(),
                                      Filter::build_range("batch_ends_at")
                                             .with_gte(JsonVal::from(now))
                                             .with_format("epoch_second")
                                             .build()
                                    ])
                                  .build();

    let company = match company_id {
      Some(company_id) => Company::find(conn, &company_id),
      None             => None,
    };

    match company {
      Some(company) => {
        // This could be a little dangerous without a backend validation.
        // We can leave as it is (but it's bruteforce-able) or otherwise
        // validating the requester by quering the DB or Honeypot itself.
        let presented_talents = Filter::build_bool()
                                       .with_must(
                                         vec![
                                           <Filter as VectorOfTerms<i32>>::build_terms(
                                             "ids", &company.presented_talents)
                                         ].into_iter()
                                          .flat_map(|x| x)
                                          .collect::<Vec<Filter>>())
                                       .build();
        vec![
          Filter::build_bool()
                 .with_should(vec![visibility_rules, presented_talents])
                 .build()
        ]
      },
      None => vec![visibility_rules]
    }
  }

  pub fn search_filters(conn: &Connection, params: &Map) -> Query {
    let company_id = match params.find(&["company_id"]) {
      Some(company_id) => i32::from_value(company_id)
                              .map(|id| vec![id])
                              .unwrap_or(vec![]),
      None => vec![]
    };

    Query::build_filtered(Filter::build_bool()
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

                                     User::visibility_filters(&conn, match company_id.is_empty() {
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
          .build()
  }
}

#[cfg(test)]
mod tests {
  use params::*;

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

  #[test]
  fn test_visibility_filters() {
    // TODO
  }

  #[test]
  fn test_search_filters() {
    // TODO
  }
}
