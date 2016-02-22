use rs_es::query::*;
use rs_es::units::JsonVal;

macro_rules! build_terms_of_array_impl {
  ($t:ty) => {
    impl BuildTermsOfArray<$t> for Filter {
      fn build_terms(key: &'static str, values: &Vec<$t>) -> Vec<Filter> {
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
  }
}

pub trait BuildTermsOfArray<T> {
  fn build_terms(key: &'static str, values: &Vec<T>) -> Vec<Filter>;
}

build_terms_of_array_impl!(i32);
build_terms_of_array_impl!(&'static str);
