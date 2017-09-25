use super::params::*;

use super::rs_es::Client;
use super::rs_es::query::Query;
use super::rs_es::operations::search::SearchHitsHitsResult;
use super::rs_es::operations::bulk::{BulkResult, Action};
use super::rs_es::operations::delete::DeleteResult;
use super::rs_es::operations::mapping::*;
use super::rs_es::error::EsError;

use resource::*;

/// The type that we use in ElasticSearch for defining a `Score`.
const ES_TYPE: &'static str = "score";

/// A collection of `Score`s.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResults {
  pub total:  u64,
  pub scores: Vec<Score>,
}

/// The representation of the score that will be indexed into ElasticSearch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Score {
  pub match_id:  String,
  pub job_id:    u32,
  pub talent_id: u32,
  pub score:     f32
}

/// Convert an ElasticSearch result into a `Score`.
impl From<SearchHitsHitsResult<Score>> for Score {
  fn from(hit: SearchHitsHitsResult<Score>) -> Score {
    *hit.source.unwrap()
  }
}

impl Score {
  pub fn search_filters(params: &Map) -> Query {
    let job_id = match params.get("job_id") {
      Some(&Value::U64(ref job_id)) => *job_id,
      _                             => 0
    };

    let talent_id = match params.get("talent_id") {
      Some(&Value::U64(ref talent_id)) => *talent_id,
      _                                => 0
    };

    Query::build_bool()
          .with_must(
             vec![
               Query::build_term("job_id", job_id).build(),
               Query::build_term("talent_id", talent_id).build()
             ])
          .build()
  }

  pub fn search(es: &mut Client, default_index: &str, params: &Map) -> SearchResults {
    let index: Vec<&str> = match params.get("index") {
      Some(&Value::String(ref index)) => vec![&index[..]],
      _                               => vec![default_index]
    };

    let result = es.search_query()
                   .with_indexes(&*index)
                   .with_query(&Score::search_filters(params))
                   .send::<Score>();

    match result {
      Ok(result) => {
        let scores: Vec<Score> = result.hits.hits.into_iter()
                                                 .map(Score::from)
                                                 .collect();

        SearchResults {
          total:  result.hits.total,
          scores: scores
        }
      },
      Err(err) => {
        error!("{:?}", err);
        SearchResults { total: 0, scores: vec![] }
      }
    }
  }

  fn delete(&self, es: &mut Client, index: &str) -> Result<DeleteResult, EsError> {
    es.delete(index, ES_TYPE, &*self.match_id)
      .send()
  }
}

impl Resource for Score {
  type Results = SearchResults;

  /// Populate the ElasticSearch index with `Vec<Score>`
  fn index(es: &mut Client, index: &str, resources: Vec<Self>) -> Result<BulkResult, EsError> {
    es.bulk(&resources.into_iter()
                      .map(|r| {
                          let match_id = r.match_id.to_owned();
                          Action::index(r).with_id(match_id)
                      })
                      .collect::<Vec<Action<Score>>>())
      .with_index(index)
      .with_doc_type(ES_TYPE)
      .send()
  }

  /// We'll call this one from `talent` as a normal function, we won't expose it outside.
  fn search(_es: &mut Client, _default_index: &str, _params: &Map) -> Self::Results {
    unimplemented!();
  }

  /// We'll call this one from `talent` as a normal function, we won't expose it outside.
  fn delete(_es: &mut Client, _id: &str, _index: &str) -> Result<DeleteResult, EsError> {
    unimplemented!();
  }

  /// We leave ES to create the mapping by inferring it from the input.
  #[allow(unused_must_use)]
  fn reset_index(_es: &mut Client, _index: &str) -> Result<MappingResult, EsError> {
    unimplemented!();
  }
}

#[cfg(test)]
mod tests {
  extern crate rs_es;
  use self::rs_es::Client;

  extern crate params;
  use self::params::*;

  use resource::*;

  use resources::{Score, Talent};
  use resources::score::SearchResults;
  use resources::tests::*;

  pub fn populate_index(mut client: &mut Client) -> bool {
    let scores = vec![
      Score {
        match_id:  "515ec9bb-0511-4464-92bb-bd21c5ed7b22".to_owned(),
        job_id:    1,
        talent_id: 10,
        score:     0.545
      }
    ];

    Score::index(&mut client, &config.es.index, scores).is_ok()
  }

  fn refresh_index(client: &mut Client) {
    client.refresh()
          .with_indexes(&[&config.es.index])
          .send()
          .unwrap();
  }

  impl SearchResults {
    pub fn match_ids(&self) -> Vec<String> {
      self.scores.iter().map(|s| s.match_id.to_owned()).collect()
    }
  }

  #[test]
  fn test_search() {
    let mut client = make_client();

    assert!(Talent::reset_index(&mut client, &*config.es.index).is_ok());
    refresh_index(&mut client);

    assert!(populate_index(&mut client));
    refresh_index(&mut client);

    // no parameters are given
    {
      let results = Score::search(&mut client, &*config.es.index, &Map::new());
      assert_eq!(0, results.total);
      assert!(results.scores.is_empty());
    }

    // given parameters have an unexpected type
    {
      let mut map = Map::new();
      map.assign("job_id", Value::String("2B".into())).unwrap();
      map.assign("talent_id", Value::String("9S".into())).unwrap();

      let results = Score::search(&mut client, &*config.es.index, &map);
      assert_eq!(0, results.total);
      assert!(results.scores.is_empty());
    }

    // job_id and talent_id are given
    {
      let mut map = Map::new();
      map.assign("job_id", Value::U64(1)).unwrap();
      map.assign("talent_id", Value::U64(10)).unwrap();

      let results = Score::search(&mut client, &*config.es.index, &map);
      assert_eq!(1, results.total);
      assert_eq!(vec!["515ec9bb-0511-4464-92bb-bd21c5ed7b22"], results.match_ids());
      assert_ne!(vec!["2a-2b-9s"], results.match_ids());
    }

    // delete between searches
    {
      let mut map = Map::new();
      map.assign("job_id", Value::U64(1)).unwrap();
      map.assign("talent_id", Value::U64(10)).unwrap();

      let results = Score::search(&mut client, &*config.es.index, &map);
      assert_eq!(1, results.total);

      let score = &results.scores[0];
      score.delete(&mut client, &*config.es.index).unwrap();

      refresh_index(&mut client);

      let results = Score::search(&mut client, &*config.es.index, &map);
      assert_eq!(0, results.total);
    }
  }
}
