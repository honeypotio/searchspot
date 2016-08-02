use rs_es::query::Query;

pub trait VectorOfMatches<T> {
  /// Extract the elements inside `Vec<T>` into `Vec<Filter>`, if present.
  /// Every element will be mapped into a `JsonVal`.
  fn build_match(key: &str, values: &Vec<T>) -> Vec<Query>;
}

impl VectorOfMatches<String> for Query {
  fn build_match(key: &str, values: &Vec<String>) -> Vec<Query> {
    if values.is_empty() {
      return vec![];
    }

    let mut queries: Vec<Query> = Vec::new();
    for value in values.iter() {
      queries.push(Query::build_match(key, value.to_owned()).build());
    }
    queries
  }
}

#[cfg(test)]
mod tests {
  use matches::*;
  use rs_es::query::Query;
  use serde_json;

  #[test]
  fn test_vector_of_matches() {
    assert!(<Query as VectorOfMatches<String>>::build_match("work_roles", &vec![])
                                               .is_empty());

    {
      let filters = <Query as VectorOfMatches<String>>::build_match(
                  "work_roles", &vec!["Fullstack".to_owned()]);
      assert_eq!(serde_json::to_string(&filters[0]).unwrap(),
                  "{\"match\":{\"work_roles\":{\"query\":\"Fullstack\"}}}".to_owned());
    }
  }
}
