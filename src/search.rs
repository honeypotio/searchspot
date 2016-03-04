use params::*;

use rustc_serialize::json::{ToJson, Json};

use std::collections::BTreeMap;

/// Struct for containing the search results and the
/// parameters given inside the query string.
pub struct SearchResult {
  pub results: Vec<u32>,
  pub params:  Map
}

impl ToJson for SearchResult {
  /// Results are returned as Json<Vec<u32>>, while params
  /// are casted to `Vec<String>`, `u32` and `String`, fallbacking
  /// one to another the conversion doesn't succeed.
  fn to_json(&self) -> Json {
    let mut values: BTreeMap<String, Json> = BTreeMap::new();

    for (key_, value) in &self.params.0 {
      let key = key_.clone();

      match Vec::<String>::from_value(value) {
        Some(value) => { values.insert(key, value.to_json()); },
        None        => {
          match u32::from_value(value) {
            Some(value) => { values.insert(key, value.to_json()); },
            None        => {
              match String::from_value(value) {
                Some(value) => { values.insert(key, value.to_json()); },
                None        => ()
              }
            }
          }
        }
      }
    }

    let mut map = BTreeMap::new();
    map.insert("results".to_string(), self.results.to_json());
    map.insert("params".to_string(),  values.to_json());

    Json::Object(map)
  }
}

#[cfg(test)]
mod tests {
  use params::*;
  use search::SearchResult;
  use rustc_serialize::json::{self, ToJson};

  #[test]
  fn test_search_result_to_json() {
    {
      let mut params = Map::new();
      params.assign("work_roles[]", Value::String("Fullstack".into())).unwrap();
      params.assign("work_roles[]", Value::String("DevOps".into())).unwrap();
      params.assign("page",         Value::I64(42)).unwrap();
      params.assign("vbb",          Value::String("vbb".into())).unwrap();

      let response = SearchResult {
        results: vec![],
        params:  params.clone()
      };

      let json_response = json::encode(&response.to_json()).unwrap();
      assert_eq!(json_response,
        "{\"params\":{\"page\":42,\"vbb\":\"vbb\",\"work_roles\":[\"Fullstack\",\"DevOps\"]},\"results\":[]}");
    }

    {
      let response = SearchResult {
        results: vec![],
        params:  Map::new()
      };

      let json_response = json::encode(&response.to_json()).unwrap();
      assert_eq!(json_response, "{\"params\":{},\"results\":[]}");
    }
  }
}
