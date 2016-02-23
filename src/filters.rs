use chrono::UTC;
use chrono::datetime::DateTime;

use company::Company;
use postgres::Connection;

use rs_es::query::Filter;
use rs_es::units::JsonVal;

pub fn visibility_filters(conn: &Connection, company_id: &Option<i32>) -> Vec<Filter> {
  let now = DateTime::timestamp(&UTC::now());

  let visibility_rules = Filter::build_bool()
                                .with_must(vec![Filter::build_term("accepted", "true")
                                                      .build(),
                                                Filter::build_range("batch_starts_at")
                                                       .with_lt(JsonVal::from(now))
                                                       .with_format("epoch_second")
                                                       .build(),
                                               Filter::build_range("batch_ends_at")
                                                       .with_gte(JsonVal::from(now))
                                                       .with_format("epoch_second")
                                                       .build()]).build();
  match *company_id {
    Some(company_id) => {
      let presented_talent_ids: Vec<i32> = Company::find(conn, &company_id).unwrap().presented_talents;

      let presented_talents = Filter::build_bool()
                                        .with_must(vec![<Filter as VectorOfTerms<i32>>::build_terms("ids", &presented_talent_ids)].into_iter()
                                                                                                                                  .flat_map(|x| x)
                                                                                                                                  .collect::<Vec<Filter>>())
                                        .build();
      vec![Filter::build_bool()
             .with_should(vec![visibility_rules, presented_talents])
             .build()]
    },
    None => vec![visibility_rules]
  }
}

macro_rules! build_vector_of_terms_impl {
  ($t:ty) => {
    impl VectorOfTerms<$t> for Filter {
      /// Extract all given items into multiple filters
      /// i.e. build_terms("field", vec![1, 2]) => vec![Filter(1), Filter(2)]
      /// This enable us to operate on these values with boolean values
      fn build_terms(key: &str, values: &Vec<$t>) -> Vec<Filter> {
        // Skip empty vectors
        if values.is_empty() {
          return vec![];
        }

        vec![Filter::build_terms(
          key,
          values.iter()
                .map(|v| JsonVal::from(*v))
                .collect::<Vec<JsonVal>>()
        ).build()]
      }
    }
  }
}

pub trait VectorOfTerms<T> {
  fn build_terms(key: &str, values: &Vec<T>) -> Vec<Filter>;
}

build_vector_of_terms_impl!(i32);
build_vector_of_terms_impl!(&'static str);
