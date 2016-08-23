use rs_es::query::Query;
use rs_es::query::full_text::MatchType;

pub trait VectorOfMatches<T> {
  /// Extract the elements inside `Vec<T>` into `Vec<Filter>`, if present.
  /// Every element will be mapped into a `JsonVal`.
  fn build_match(key: &str, values: &Vec<T>, match_type: Option<MatchType>) -> Vec<Query>;
}

impl VectorOfMatches<String> for Query {
  fn build_match(key: &str, values: &Vec<String>, match_type: Option<MatchType>) -> Vec<Query> {
    if values.is_empty() {
      return vec![];
    }

    let mut queries: Vec<Query> = Vec::new();

    for value in values.iter() {
      queries.push(
        match match_type.to_owned() {
          Some(t) => Query::build_match(key, value.to_owned()).with_type(t),
          None    => Query::build_match(key, value.to_owned())
        }.build());
    }

    queries
  }
}

#[cfg(test)]
mod tests {
  use matches::*;
  use rs_es::query::Query;
  use rs_es::query::full_text::MatchType;
  use serde_json;

  #[test]
  fn test_vector_of_matches() {
    assert!(<Query as VectorOfMatches<String>>::build_match("work_roles", &vec![], None)
                                               .is_empty());

    {
      let filters = <Query as VectorOfMatches<String>>::build_match(
                  "work_roles", &vec!["Fullstack".to_owned()], None);
      assert_eq!(serde_json::to_string(&filters[0]).unwrap(),
                  "{\"match\":{\"work_roles\":{\"query\":\"Fullstack\"}}}".to_owned());
    }

    {
      let filters = <Query as VectorOfMatches<String>>::build_match(
                  "work_roles", &vec!["Fullstack".to_owned()], Some(MatchType::Phrase));
      assert_eq!(serde_json::to_string(&filters[0]).unwrap(),
                  "{\"match\":{\"work_roles\":{\"query\":\"Fullstack\",\"type\":\"phrase\"}}}".to_owned());
    }
  }
}
