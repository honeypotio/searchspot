use chrono::UTC;
use chrono::datetime::DateTime;

use company::Company;

use postgres::Connection;

use rs_es::query::Filter;
use rs_es::units::JsonVal;

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

pub trait VectorOfTerms<T> {
  fn build_terms(key: &str, values: &Vec<T>) -> Vec<Filter>;
}

macro_rules! build_vector_of_terms_impl {
  ($t:ty) => {
    impl VectorOfTerms<$t> for Filter {
      /// Extract all given items into multiple filters (if present)
      /// i.e. build_terms("field", vec![1, 2]) => vec![Filter(1), Filter(2)]
      /// This enable us to operate on these values with boolean values
      fn build_terms(key: &str, values: &Vec<$t>) -> Vec<Filter> {
        if values.is_empty() {
          return vec![];
        }

        vec![
          Filter::build_terms(key, values.iter()
                                         .map(|v| JsonVal::from(v.clone()))
                                         .collect::<Vec<JsonVal>>()).build()
        ]
      }
    }
  }
}

build_vector_of_terms_impl!(i32);
build_vector_of_terms_impl!(String);

#[cfg(test)]
mod tests {
  use filters::*;
  use rs_es::query::Filter;
  use rustc_serialize::json::ToJson;

  #[test]
  fn test_vector_of_terms() {
    assert!(<Filter as VectorOfTerms<String>>::build_terms("work_roles", &vec![])
                                              .is_empty());

    {
      let filters = <Filter as VectorOfTerms<String>>::build_terms(
                  "work_roles", &vec![String::from("Fullstack")]);
      assert_eq!(filters[0].to_json().to_string(),
        String::from("{\"terms\":{\"work_roles\":[\"Fullstack\"]}}"));
    }

    {
      let filters = <Filter as VectorOfTerms<i32>>::build_terms(
                  "work_roles", &vec![1]);
      assert_eq!(filters[0].to_json().to_string(),
        String::from("{\"terms\":{\"work_roles\":[1]}}"));
    }
  }

  #[test]
  fn test_visibility_filters() {
    // TODO
  }
}
