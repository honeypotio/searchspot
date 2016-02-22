use rs_es::query::*;
use rs_es::units::JsonVal;

pub trait BuildTermsOfArray<T> {
  fn build_terms(key: &'static str, values: &Vec<T>) -> Vec<Filter>;
}

impl BuildTermsOfArray<i32> for Filter {
  fn build_terms(key: &'static str, values: &Vec<i32>) -> Vec<Filter> {
    // An empty `values` does nothing so that you don't need to assert its presence
    if values.is_empty() {
      return vec![];
    }

    let mut terms: Vec<Filter> = Vec::new();

    for value in values {
      let term = Filter::build_terms(
        key,
        vec![JsonVal::from(*value)]
      ).build();

      terms.push(term);
    }

    terms
  }
}

impl BuildTermsOfArray<&'static str> for Filter {
  fn build_terms(key: &'static str, values: &Vec<&'static str>) -> Vec<Filter> {
    // An empty `values` does nothing so that you don't need to assert its presence
    if values.is_empty() {
      return vec![];
    }

    let mut terms: Vec<Filter> = Vec::new();

    for value in values {
      let term = Filter::build_terms(
        key,
        vec![JsonVal::from(*value)]
      ).build();

      terms.push(term);
    }

    terms
  }
}
