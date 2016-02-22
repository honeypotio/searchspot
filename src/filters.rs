use rs_es::query::*;
use rs_es::units::JsonVal;

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

pub trait VectorOfTerms<T> {
  fn build_terms(key: &str, values: &Vec<T>) -> Vec<Filter>;
}

build_vector_of_terms_impl!(i32);
build_vector_of_terms_impl!(&'static str);
