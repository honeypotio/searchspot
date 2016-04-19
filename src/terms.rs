use rs_es::query::Filter;
use rs_es::units::JsonVal;

pub trait VectorOfTerms<T> {
  /// Extract the elements inside `Vec<T>` into `Vec<Filter>`, if present.
  /// Every element will be mapped into a `JsonVal`.
  fn build_terms(key: &str, values: &Vec<T>) -> Vec<Filter>;
}

macro_rules! build_vector_of_terms_impl {
  ($t:ty) => {
    impl VectorOfTerms<$t> for Filter {
      fn build_terms(key: &str, values: &Vec<$t>) -> Vec<Filter> {
        if values.is_empty() {
          return vec![];
        }

        vec![
          Filter::build_terms(key, values.iter()
                                         .map(|v| JsonVal::from(v.to_owned()))
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
  use terms::*;
  use rs_es::query::Filter;
  use rustc_serialize::json::ToJson;

  #[test]
  fn test_vector_of_terms() {
    assert!(<Filter as VectorOfTerms<String>>::build_terms("work_roles", &vec![])
                                              .is_empty());

    {
      let filters = <Filter as VectorOfTerms<String>>::build_terms(
                      "work_roles", &vec!["Fullstack".to_owned()]);
      assert_eq!(filters[0].to_json().to_string(),
                  "{\"terms\":{\"work_roles\":[\"Fullstack\"]}}".to_owned());
    }

    {
      let filters = <Filter as VectorOfTerms<i32>>::build_terms(
                  "work_roles", &vec![1]);
      assert_eq!(filters[0].to_json().to_string(), "{\"terms\":{\"work_roles\":[1]}}".to_owned());
    }
  }
}
