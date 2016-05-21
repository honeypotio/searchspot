/// Struct for containing the search results
#[derive(Serialize, Deserialize)]
pub struct SearchResult {
  pub results: Vec<u32>
}

#[cfg(test)]
mod tests {
  use search::SearchResult;
  use serde_json;

  #[test]
  fn test_search_result_to_json() {
    {
      let response = SearchResult {
        results: vec![]
      };

      let json_response = serde_json::to_string(&response).unwrap();
      assert_eq!(json_response, "{\"results\":[]}");
    }
  }
}
