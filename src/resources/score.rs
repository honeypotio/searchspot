use params::Map;

use rs_es::Client;
use rs_es::query::Query;
use rs_es::operations::search::SearchHitsHitsResult;
use rs_es::operations::bulk::{BulkResult, Action};
use rs_es::operations::delete::DeleteResult;
use rs_es::operations::mapping::MappingResult;
use rs_es::error::EsError;

use resource::Resource;

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
  pub request_id:  String,
  pub person_id:   Option<String>,
  pub company_id:  Option<String>,
  pub position_id: Option<String>,
  pub job_id:    u32,
  pub talent_id: u32,
  pub score:     f32
}

#[derive(Default, Clone)]
pub struct SearchBuilder {
  pub job_id:    Option<u32>,
  pub talent_id: Option<u32>
}

impl SearchBuilder {
  pub fn new() -> SearchBuilder {
    SearchBuilder::default()
  }

  pub fn with_job_id(&mut self, job_id: u32) -> &mut SearchBuilder {
    self.job_id = Some(job_id);
    self
  }

  pub fn with_talent_id(&mut self, talent_id: u32) -> &mut SearchBuilder {
    self.talent_id = Some(talent_id);
    self
  }

  pub fn build(&self) -> SearchBuilder {
    self.to_owned()
  }

  pub fn to_query(&self) -> Query {
    let mut terms = vec![];

    if let Some(job_id) = self.job_id {
      terms.push(
        Query::build_term("job_id", job_id).build()
      );
    }

    if let Some(talent_id) = self.talent_id {
      terms.push(
        Query::build_term("talent_id", talent_id).build()
      );
    }

    Query::build_bool()
          .with_must(terms)
          .build()
  }
}

/// Convert an ElasticSearch result into a `Score`.
impl From<SearchHitsHitsResult<Score>> for Score {
  fn from(hit: SearchHitsHitsResult<Score>) -> Score {
    *hit.source.unwrap()
  }
}

impl Score {
  pub fn search(es: &mut Client, index: &str, search_builder: &SearchBuilder) -> SearchResults {
    let result = es.search_query()
                   .with_indexes(&[index])
                   .with_query(&search_builder.to_query())
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

  pub fn delete(&self, es: &mut Client, index: &str) -> Result<DeleteResult, EsError> {
    es.delete(index, ES_TYPE, &*self.request_id)
      .send()
  }
}

impl Resource for Score {
  type Results = SearchResults;

  /// Populate the ElasticSearch index with `Vec<Score>`
  fn index(es: &mut Client, index: &str, resources: Vec<Self>) -> Result<BulkResult, EsError> {
    es.bulk(&resources.into_iter()
                      .map(|r| {
                          let request_id = r.request_id.to_owned();
                          Action::index(r).with_id(request_id)
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
  fn reset_index(_es: &mut Client, _index: &str) -> Result<MappingResult, EsError> {
    unimplemented!();
  }
}

#[cfg(test)]
mod tests {
  use rs_es::Client;

  use resource::Resource;

  use resources::{Score, Talent};
  use resources::score::{SearchBuilder, SearchResults};
  use resources::tests::{make_client, CONFIG, refresh_index};

  pub fn populate_index(mut client: &mut Client, index: &str) -> bool {
    let scores = vec![
      Score {
        request_id:  "515ec9bb-0511-4464-92bb-bd21c5ed7b22".to_owned(),
        person_id:   Some("5801f578-a3bc-40ee-94fd-b437f94f00d5".to_owned()),
        company_id:  Some("5f97ba87-463c-4531-b35a-f4626a3d8998".to_owned()),
        position_id: Some("6214ab8d26e3f79571d922ca269d5749".to_owned()),
        job_id:    1,
        talent_id: 1,
        score:     0.545
      },

      Score {
        request_id:  "9ac871a8-d936-41d8-bd35-9bc3c0c5be42".to_owned(),
        person_id:   None,
        company_id:  None,
        position_id: None,
        job_id:    1,
        talent_id: 2,
        score:     0.442
      }
    ];

    Score::index(&mut client, &index, scores).is_ok()
  }

  impl SearchResults {
    pub fn request_ids(&self) -> Vec<String> {
      self.scores.iter().map(|s| s.request_id.to_owned()).collect()
    }
  }

  #[test]
  fn test_search() {
    let mut client = make_client();
    let     index  = format!("{}_{}", CONFIG.es.index, "score");

    if let Err(_) = Talent::reset_index(&mut client, &*index) {
      let _ = Talent::reset_index(&mut client, &*index);
    }

    refresh_index(&mut client, &*index);

    assert!(populate_index(&mut client, &*index));
    refresh_index(&mut client, &*index);

    // no parameters are given
    {
      let search  = SearchBuilder::new().build();
      let results = Score::search(&mut client, &*index, &search);
      assert_eq!(2, results.total);
    }

    // job_id is given
    {
      let search  = SearchBuilder::new().with_job_id(1).build();
      let results = Score::search(&mut client, &*index, &search);
      assert_eq!(2, results.total);
    }

    // both job_id and talent_id are given
    {
      let search = SearchBuilder::new()
                                 .with_talent_id(1)
                                 .with_job_id(1)
                                 .build();

      let results = Score::search(&mut client, &*index, &search);
      assert_eq!(1, results.total);
      assert_eq!(vec!["515ec9bb-0511-4464-92bb-bd21c5ed7b22"], results.request_ids());
    }

    // delete between searches
    {
      let search  = SearchBuilder::new().with_talent_id(1).build();
      let results = Score::search(&mut client, &*index, &search);
      assert_eq!(1, results.total);

      results.scores[0].delete(&mut client, &*index).unwrap();

      refresh_index(&mut client, &*index);

      let results = Score::search(&mut client, &*index, &search);
      assert_eq!(0, results.total);
    }
  }
}
