use super::chrono::prelude::*;

use super::params::*;

use super::rs_es::Client;
use super::rs_es::query::Query;
use super::rs_es::operations::search::{Sort, SortField, Order, SearchHitsHitsResult};
use super::rs_es::operations::bulk::{BulkResult, Action};
use super::rs_es::operations::delete::DeleteResult;
use super::rs_es::operations::mapping::*;
use super::rs_es::query::full_text::MatchQueryType;
use super::rs_es::error::EsError;
use super::rs_es::operations::search::highlight::*;

use terms::VectorOfTerms;
use resource::*;
use resources::score::Score;
use resources::score::SearchBuilder as ScoreSearchBuilder;

/// The type that we use in ElasticSearch for defining a `Talent`.
const ES_TYPE: &'static str = "talent";

/// A collection of `SearchResult`s.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResults {
  pub total:   u64,
  pub talents: Vec<SearchResult>,
}

/// A single search result returned by ElasticSearch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
  pub talent:    FoundTalent,
  pub score:     Option<Score>,
  pub highlight: Option<HighlightResult>
}

impl SearchResult {
  fn with_score(&mut self, score: Option<Score>) -> SearchResult {
    self.score = score;
    self.to_owned()
  }
}

/// Convert an ElasticSearch result into a `SearchResult`.
impl From<SearchHitsHitsResult<Talent>> for SearchResult {
  fn from(result: SearchHitsHitsResult<Talent>) -> SearchResult {
    SearchResult {
      talent:    result.source.unwrap().into(),
      score:     None,
      highlight: result.highlight
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SalaryExpectations {
  pub minimum:  Option<u64>,
  pub maximum:  Option<u64>,
  pub currency: String,
  pub city:     String
}

/// A representation of `Talent` with limited fields.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FoundTalent {
  pub id:                  u32,
  pub headline:            String,
  pub avatar_url:          String,
  pub work_locations:      Vec<String>,
  pub current_location:    String,
  pub salary_expectations: Vec<SalaryExpectations>,
  pub roles_experiences:   Vec<RolesExperience>,
  pub latest_position:     String,
  pub batch_starts_at:     String
}

/// A struct that joins `desired_work_roles` and `desired_work_roles_experience`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RolesExperience {
  pub role:       String,
  pub experience: String
}

impl RolesExperience {
  fn new(role: &str, experience: Option<&String>) -> RolesExperience {
    RolesExperience {
      role:       role.to_owned(),
      experience: experience.map(|e| e.to_owned()).unwrap_or("".to_owned())
    }
  }
}

/// Convert a `Box<Talent>` returned by ElasticSearch into a `FoundTalent`.
impl From<Box<Talent>> for FoundTalent {
  fn from(talent: Box<Talent>) -> FoundTalent {
    let mut roles_experiences = vec![];

    for (i, role) in talent.desired_work_roles.iter().enumerate() {
      let experience = talent.desired_work_roles_experience.get(i);
      roles_experiences.push(RolesExperience::new(role, experience));
    }

    FoundTalent {
      id:                  talent.id,
      headline:            talent.headline.to_owned(),
      avatar_url:          talent.avatar_url.to_owned(),
      work_locations:      talent.work_locations.to_owned(),
      current_location:    talent.current_location.to_owned(),
      salary_expectations: talent.salary_expectations.to_owned(),
      roles_experiences:   roles_experiences,
      latest_position:     talent.latest_position.to_owned(),
      batch_starts_at:     talent.batch_starts_at.to_owned()
    }
  }
}

/// The talent that will be indexed into ElasticSearch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Talent {
  pub id:                            u32,
  pub accepted:                      bool,
  pub desired_work_roles:            Vec<String>,
  pub desired_work_roles_vanilla:    Option<Vec<String>>, // not processed by ES
  pub desired_work_roles_experience: Vec<String>, // experience in the desired work roles
  pub professional_experience:       String, // i.e. 2..6
  pub work_locations:                Vec<String>, // wants to work in
  pub current_location:              String, // where the talent is based in
  pub work_authorization:            String, // yes/no/unsure (visa)
  pub skills:                        Vec<String>,
  pub summary:                       String,
  pub headline:                      String,
  pub contacted_company_ids:         Vec<u32>, // contacted companies
  pub batch_starts_at:               String,
  pub batch_ends_at:                 String,
  pub added_to_batch_at:             String,
  pub weight:                        i32,
  pub blocked_companies:             Vec<u32>,
  pub work_experiences:              Vec<String>, // past work experiences (i.e. ["Frontend developer", "SysAdmin"])
  pub avatar_url:                    String,
  pub salary_expectations:           Vec<SalaryExpectations>,
  pub latest_position:               String, // the very last experience_entries#position
  pub languages:                     Vec<String>
}

impl Talent {
  /// Return a `Vec<Query>` with visibility criteria for the talents.
  /// The `epoch` must be given as `I64` (UNIX time in seconds) and is
  /// the range in which batches are searched.
  /// If `presented_talents` is provided, talents who match the IDs
  /// contained there skip the standard visibility criteria.
  ///
  /// Basically, the talents must be accepted into the platform and must be
  /// inside a living batch to match the visibility criteria.
  pub fn visibility_filters(epoch: &str, presented_talents: Vec<i32>, date_filter_present: bool) -> Vec<Query> {
    let visibility_rules;

    if date_filter_present {
      visibility_rules = Query::build_bool()
                                   .with_must(
                                      vec![
                                        Query::build_term("accepted", true)
                                              .build(),
                                        Query::build_term("batch_starts_at", epoch)
                                              .build()
                                      ])
                                   .build();
    } else {
      visibility_rules = Query::build_bool()
                                   .with_must(
                                      vec![
                                        Query::build_term("accepted", true)
                                              .build(),
                                        Query::build_range("batch_starts_at")
                                              .with_lte(epoch)
                                              .with_format("dateOptionalTime")
                                              .build(),
                                        Query::build_range("batch_ends_at")
                                              .with_gte(epoch)
                                              .with_format("dateOptionalTime")
                                              .build()
                                      ])
                                   .build();
    }

    if !presented_talents.is_empty() {
      let presented_talents_filters = Query::build_bool()
                                            .with_must(
                                              vec![
                                                <Query as VectorOfTerms<i32>>::build_terms(
                                                  "ids", &presented_talents)
                                              ].into_iter()
                                               .flat_map(|x| x)
                                               .collect::<Vec<Query>>())
                                            .build();
      vec![
        Query::build_bool()
              .with_should(vec![visibility_rules, presented_talents_filters])
              .build()
      ]
    }
    else {
      vec![visibility_rules]
    }
  }

  /// Given parameters inside the query string mapped inside a `Map`,
  /// and the `epoch` (defined as UNIX time in seconds) for batches,
  /// return a `Query` for ElasticSearch.
  ///
  /// Considering a single row, the terms inside there are ORred,
  /// while through the rows there is an AND.
  /// I.e.: given ["Fullstack", "DevOps"] as `desired_work_roles`, found talents
  /// will present at least one of these roles), but both `desired_work_roles`
  /// and `work_location`, if provided, must be matched successfully.
  pub fn search_filters(params: &Map, epoch: &str) -> Query {
    let company_id = i32_vec_from_params!(params, "company_id");
    let date_filter_present = params.get("epoch") != None;

    Query::build_bool()
          .with_must(
             vec![
                match Talent::full_text_search(params) {
                  Some(keywords) => vec![keywords],
                  None           => vec![]
                },

                vec![
                  Query::build_bool()
                        .with_must(
                           vec_from_params!(params, "languages").into_iter().map(|language: String| {
                             Query::build_term("languages", language).build()
                           }).collect::<Vec<Query>>()
                        )
                      .build()],

               <Query as VectorOfTerms<String>>::build_terms(
                 "desired_work_roles_vanilla", &vec_from_params!(params, "desired_work_roles")),

               <Query as VectorOfTerms<String>>::build_terms(
                 "professional_experience", &vec_from_params!(params, "professional_experience")),

               <Query as VectorOfTerms<String>>::build_terms(
                 "work_authorization", &vec_from_params!(params, "work_authorization")),

               <Query as VectorOfTerms<String>>::build_terms(
                 "work_locations", &vec_from_params!(params, "work_locations")),

               <Query as VectorOfTerms<i32>>::build_terms(
                 "id", &vec_from_params!(params, "ids")),

               Talent::visibility_filters(epoch,
                 i32_vec_from_params!(params, "presented_talents"),
                 date_filter_present)
               ].into_iter()
                .flat_map(|x| x)
                .collect::<Vec<Query>>())
                .with_must_not(
                   vec![
                     <Query as VectorOfTerms<i32>>::build_terms(
                       "contacted_company_ids", &company_id),

                     <Query as VectorOfTerms<i32>>::build_terms(
                       "blocked_companies", &company_id),

                     <Query as VectorOfTerms<i32>>::build_terms(
                       "id", &vec_from_params!(params, "contacted_talents"))
                   ].into_iter()
                    .flat_map(|x| x)
                    .collect::<Vec<Query>>())
          .build()
  }

  pub fn full_text_search(params: &Map) -> Option<Query> {
    match params.get("keywords") {
      Some(&Value::String(ref keywords)) => {
        if keywords.is_empty() {
          return None;
        }

        Some(Query::build_multi_match(
                vec![
                  "skills".to_owned(),
                  "summary".to_owned(),
                  "headline".to_owned(),
                  "desired_work_roles".to_owned(),
                  "work_experiences".to_owned()
                ], keywords.to_owned())
            .with_type(MatchQueryType::CrossFields)
            .with_tie_breaker(0.0)
            .build())
      },
      _ => None
    }
  }

  /// Return a `Sort` that makes values be sorted for given fields, descendently.
  pub fn sorting_criteria() -> Sort {
    Sort::new(
      vec![
        SortField::new("batch_starts_at",   Some(Order::Desc)).with_unmapped_type("date").build(),
        SortField::new("weight",            Some(Order::Desc)).with_unmapped_type("integer").build(),
        SortField::new("added_to_batch_at", Some(Order::Desc)).with_unmapped_type("date").build()
      ])
  }
}

impl Resource for Talent {
  type Results = SearchResults;

  /// Populate the ElasticSearch index with `Vec<Talent>`
  fn index(es: &mut Client, index: &str, resources: Vec<Self>) -> Result<BulkResult, EsError> {
    es.bulk(&resources.into_iter()
                      .map(|mut r| {
                          let id = r.id.to_string();
                          r.desired_work_roles_vanilla = Some(r.desired_work_roles.to_owned());
                          Action::index(r).with_id(id)
                      })
                      .collect::<Vec<Action<Talent>>>())
      .with_index(index)
      .with_doc_type(ES_TYPE)
      .send()
  }

  /// Query ElasticSearch on given `indexes` and `params` and return the IDs of
  /// the found talents.
  fn search(es: &mut Client, default_index: &str, params: &Map) -> Self::Results {
    let epoch = match params.get("epoch") {
      Some(&Value::String(ref epoch)) => epoch.to_owned(),
      _                               => Utc::now().to_rfc3339()
    };

    let index: Vec<&str> = match params.get("index") {
      Some(&Value::String(ref index)) => vec![&index[..]],
      _                               => vec![default_index]
    };

    let keywords_present = match params.get("keywords") {
      Some(keywords) => match keywords {
        &Value::String(ref keywords) => !keywords.is_empty(),
        _                            => false
      },
      None => false
    };

    let offset = match params.get("offset") {
      Some(&Value::U64(ref offset)) => *offset,
      _                             => 0
    };

    let per_page = match params.get("per_page") {
      Some(&Value::U64(ref per_page)) => *per_page,
      _                               => 10
    };

    let result = if keywords_present {
      let mut highlight = Highlight::new().with_encoder(Encoders::HTML)
                                          .with_pre_tags(vec!["".to_owned()])
                                          .with_post_tags(vec!["".to_owned()])
                                          .to_owned();
      let settings = Setting::new().with_type(SettingTypes::Plain)
                                   .with_term_vector(TermVector::WithPositionsOffsets)
                                   .with_fragment_size(1)
                                   .to_owned();
      highlight.add_setting("skills".to_owned(),  settings.clone());
      highlight.add_setting("summary".to_owned(), settings.clone());
      highlight.add_setting("headline".to_owned(), settings.clone());
      highlight.add_setting("desired_work_roles".to_owned(), settings.clone());
      highlight.add_setting("work_experiences".to_owned(), settings);

      es.search_query()
        .with_indexes(&*index)
        .with_query(&Talent::search_filters(params, &*epoch))
        .with_highlight(&highlight)
        .with_from(offset)
        .with_size(per_page)
        .with_min_score(0.56)
        .with_track_scores(true)
        .send::<Talent>()
    }
    else {
      es.search_query()
        .with_indexes(&*index)
        .with_query(&Talent::search_filters(params, &*epoch))
        .with_sort(&Talent::sorting_criteria())
        .with_from(offset)
        .with_size(per_page)
        .send::<Talent>()
    };

    match result {
      Ok(result) => {
        let total = result.hits.total;

        if total == 0 {
          return SearchResults { total: 0, talents: vec![] };
        }

        let source_from_job_id = |mut result: SearchResult| {
          match params.get("job_id") {
            Some(&Value::U64(ref job_id)) => {
              let search = ScoreSearchBuilder::new()
                                              .with_job_id(*job_id as u32)
                                              .with_talent_id(result.talent.id)
                                              .build();
              let results = Score::search(es, &index[0], &search);
              result.with_score(results.scores.get(0).cloned())
            },
            _ => result
          }
        };

        let results: Vec<SearchResult> = result.hits.hits.into_iter()
                                                         .map(SearchResult::from)
                                                         .map(source_from_job_id)
                                                         .collect();

        SearchResults { total: total, talents: results }
      },
      Err(err) => {
        error!("{:?}", err);
        SearchResults { total: 0, talents: vec![] }
      }
    }
  }

  /// Delete the talent associated to given id.
  fn delete(es: &mut Client, id: &str, index: &str) -> Result<DeleteResult, EsError> {
    let search = ScoreSearchBuilder::new()
                                    .with_talent_id(id.parse().unwrap())
                                    .build();
    Score::search(es, index, &search).scores.iter()
                                            .for_each(|s| { let _ = s.delete(es, index); });

    es.delete(index, ES_TYPE, id)
      .send()
  }

  /// Reset the given index. All the data will be destroyed and then the index
  /// will be created again. The map that will be used is hardcoded.
  #[allow(unused_must_use)]
  fn reset_index(mut es: &mut Client, index: &str) -> Result<MappingResult, EsError> {
    let mapping = hashmap! {
      ES_TYPE => hashmap! {
        "id" => hashmap! {
          "type"  => "integer",
          "index" => "not_analyzed"
        },

        "desired_work_roles" => hashmap! {
          "type"            => "string",
          "analyzer"        => "trigrams",
          "search_analyzer" => "words"
        },

        "desired_work_roles_vanilla" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        "desired_work_roles_experience" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        "professional_experience" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        "work_locations" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        "languages" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        "current_location" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        "work_authorization" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        "skills" => hashmap! {
          "type"            => "string",
          "analyzer"        => "trigrams",
          "search_analyzer" => "words"
        },

        "summary" => hashmap! {
          "type"            => "string",
          "analyzer"        => "trigrams",
          "search_analyzer" => "words",
          "boost"           => "2.0",
        },

        "headline" => hashmap! {
          "type"            => "string",
          "analyzer"        => "trigrams",
          "search_analyzer" => "words",
          "boost"           => "2.0"
        },

        "work_experiences" => hashmap! {
          "type"            => "string",
          "analyzer"        => "trigrams",
          "search_analyzer" => "words"
        },

        "contacted_company_ids" => hashmap! {
          "type"  => "integer",
          "index" => "not_analyzed"
        },

        "accepted" => hashmap! {
          "type"  => "boolean",
          "index" => "not_analyzed"
        },

        "batch_starts_at" => hashmap! {
          "type"   => "date",
          "format" => "dateOptionalTime",
          "index"  => "not_analyzed"
        },

        "batch_ends_at" => hashmap! {
          "type"   => "date",
          "format" => "dateOptionalTime",
          "index"  => "not_analyzed"
        },

        "added_to_batch_at" => hashmap! {
          "type"   => "date",
          "format" => "dateOptionalTime",
          "index"  => "not_analyzed"
        },

        "weight" => hashmap! {
          "type"  => "integer",
          "index" => "not_analyzed"
        },

        "blocked_companies" => hashmap! {
          "type"  => "integer",
          "index" => "not_analyzed"
        },

        "avatar_url" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        // salary_expectations should be inferred by
        // ES as we lack of multi-field mapping right now

        "latest_position" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        }
      }
    };

    let settings = Settings {
      number_of_shards: 1,

      analysis: Analysis {
        filter: json!({
          "trigrams_filter": {
            "type": "ngram",
            "min_gram": 2,
            "max_gram": 20
          },

          "words_splitter": {
            "type": "word_delimiter",
            "preserve_original": true,
            "catenate_all": true
          },

          "english_words_filter": {
            "type": "stop",
            "stopwords": "_english_"
          },

          "tech_words_filter": {
            "type": "stop",
            "stopwords": ["js"]
          }
        }).as_object().unwrap().to_owned(),
        analyzer: json!({
          "trigrams": { // index time
            "type": "custom",
            "tokenizer": "whitespace",
            "filter": ["lowercase", "words_splitter", "trigrams_filter",
                       "english_words_filter", "tech_words_filter"]
          },

          "words": { // query time
            "type": "custom",
            "tokenizer": "keyword",
            "filter": ["lowercase", "words_splitter", "english_words_filter",
                       "tech_words_filter"]
          }
        }).as_object().unwrap().to_owned()
      }
    };

    es.delete_index(index);

    MappingOperation::new(&mut es, index)
      .with_mapping(&mapping)
      .with_settings(&settings)
      .send()
  }
}

#[cfg(test)]
mod tests {
  extern crate serde_json;

  extern crate chrono;
  use self::chrono::prelude::*;

  extern crate rs_es;
  use self::rs_es::Client;
  use self::rs_es::operations::search::highlight::HighlightResult;

  extern crate params;
  use self::params::*;

  use resource::*;

  use resources::Talent;
  use resources::talent::{SalaryExpectations, SearchResults};
  use resources::score::Score;
  use resources::score::SearchBuilder as ScoreSearchBuilder;
  use resources::tests::*;

  macro_rules! epoch_from_year {
    ($year:expr) => {
      Utc.datetime_from_str(&format!("{}-01-01 12:00:00", $year),
        "%Y-%m-%d %H:%M:%S").unwrap().to_rfc3339()
    }
  }

  impl SearchResults {
    pub fn ids(&self) -> Vec<u32> {
      self.talents.iter().map(|r| r.talent.id).collect()
    }

    pub fn highlights(&self) -> Vec<Option<HighlightResult>> {
      self.talents.iter().map(|r| r.highlight.clone()).collect()
    }

    pub fn scores(&self) -> Vec<Option<Score>> {
      self.talents.iter().map(|r| r.score.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
      self.talents.is_empty()
    }
  }

  impl SalaryExpectations {
    fn new(minimum: u64, maximum: u64, currency: &str, city: &str) -> SalaryExpectations {
      SalaryExpectations {
        minimum:  Some(minimum),
        maximum:  Some(maximum),
        currency: currency.to_owned(),
        city:     city.to_owned()
      }
    }
  }

  pub fn populate_index(mut client: &mut Client) -> bool {
    let talents = vec![
      Talent {
        id:                            1,
        accepted:                      true,
        desired_work_roles:            vec![],
        desired_work_roles_vanilla:    None,
        desired_work_roles_experience: vec![],
        professional_experience:       "1..2".to_owned(),
        work_locations:                vec!["Berlin".to_owned()],
        current_location:              "Berlin".to_owned(),
        work_authorization:            "yes".to_owned(),
        skills:                        vec!["Rust".to_owned(), "HTML5".to_owned(), "HTML".to_owned()],
        summary:                       "I'm a senior Rust developer and sometimes I do also HTML.".to_owned(),
        headline:                      "Backend developer with Rust experience".to_owned(),
        work_experiences:              vec!["Database Administrator".to_owned()],
        contacted_company_ids:         vec![],
        batch_starts_at:               epoch_from_year!("2006"),
        batch_ends_at:                 epoch_from_year!("2020"),
        added_to_batch_at:             epoch_from_year!("2006"),
        weight:                        -5,
        blocked_companies:             vec![],
        avatar_url:                    "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250".to_owned(),
        salary_expectations:           vec![SalaryExpectations::new(40_000, 50_000, "EUR", "Berlin")],
        latest_position:               "Developer".to_owned(),
        languages:                     vec!["Italian".to_owned()]
      },

      Talent {
        id:                            2,
        accepted:                      true,
        desired_work_roles:            vec![],
        desired_work_roles_vanilla:    None,
        desired_work_roles_experience: vec![],
        professional_experience:       "8+".to_owned(),
        work_locations:                vec!["Rome".to_owned(),"Berlin".to_owned()],
        current_location:              "Berlin".to_owned(),
        work_authorization:            "yes".to_owned(),
        skills:                        vec!["Rust".to_owned(), "HTML5".to_owned(), "Java".to_owned()],
        summary:                       "I'm a java dev with some tricks up my sleeves".to_owned(),
        headline:                      "Senior Java engineer".to_owned(),
        work_experiences:              vec![],
        contacted_company_ids:         vec![],
        batch_starts_at:               epoch_from_year!("2006"),
        batch_ends_at:                 epoch_from_year!("2020"),
        added_to_batch_at:             epoch_from_year!("2006"),
        weight:                        6,
        blocked_companies:             vec![22],
        avatar_url:                    "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250".to_owned(),
        salary_expectations:           vec![],
        latest_position:               "".to_owned(),
        languages:                     vec!["German".to_owned(), "English".to_owned()]
      },

      Talent {
        id:                            3,
        accepted:                      false,
        desired_work_roles:            vec![],
        desired_work_roles_vanilla:    None,
        desired_work_roles_experience: vec![],
        professional_experience:       "1..2".to_owned(),
        work_locations:                vec!["Berlin".to_owned()],
        current_location:              "Berlin".to_owned(),
        work_authorization:            "yes".to_owned(),
        skills:                        vec![],
        summary:                       "".to_owned(),
        headline:                      "".to_owned(),
        work_experiences:              vec![],
        contacted_company_ids:         vec![],
        batch_starts_at:               epoch_from_year!("2007"),
        batch_ends_at:                 epoch_from_year!("2020"),
        added_to_batch_at:             epoch_from_year!("2011"),
        weight:                        6,
        blocked_companies:             vec![],
        avatar_url:                    "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250".to_owned(),
        salary_expectations:           vec![],
        latest_position:               "".to_owned(),
        languages:                     vec!["English".to_owned()]
      },

      Talent {
        id:                            4,
        accepted:                      true,
        desired_work_roles:            vec!["Fullstack".to_owned(), "DevOps".to_owned()],
        desired_work_roles_vanilla:    None,
        desired_work_roles_experience: vec!["2..3".to_owned(), "5".to_owned()],
        professional_experience:       "1..2".to_owned(),
        work_locations:                vec!["Berlin".to_owned()],
        current_location:              "Berlin".to_owned(),
        work_authorization:            "no".to_owned(),
        skills:                        vec!["ClojureScript".to_owned(), "C++".to_owned(), "React.js".to_owned()],
        summary:                       "ClojureScript right now, previously C++".to_owned(),
        headline:                      "Senior fullstack developer with sysadmin skills".to_owned(),
        work_experiences:              vec!["Backend Engineer".to_owned(), "Database Administrator".to_owned()],
        contacted_company_ids:         vec![6],
        batch_starts_at:               epoch_from_year!("2008"),
        batch_ends_at:                 epoch_from_year!("2020"),
        added_to_batch_at:             epoch_from_year!("2011"),
        weight:                        0,
        blocked_companies:             vec![],
        avatar_url:                    "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250".to_owned(),
        salary_expectations:           vec![],
        latest_position:               "".to_owned(),
        languages:                     vec!["English".to_owned()]
      },

      Talent {
        id:                            5,
        accepted:                      true,
        desired_work_roles:            vec!["Fullstack".to_owned(), "DevOps".to_owned()],
        desired_work_roles_vanilla:    None,
        desired_work_roles_experience: vec!["2..3".to_owned(), "5".to_owned()],
        professional_experience:       "1..2".to_owned(),
        work_locations:                vec!["Berlin".to_owned()],
        current_location:              "Berlin".to_owned(),
        work_authorization:            "yes".to_owned(),
        skills:                        vec!["JavaScript".to_owned(), "C++".to_owned(), "Ember.js".to_owned()],
        summary:                       "C++ and frontend dev. HTML, C++, JavaScript and C#. Did I say C++?".to_owned(),
        headline:                      "Amazing C developer".to_owned(),
        work_experiences:              vec![],
        contacted_company_ids:         vec![6],
        batch_starts_at:               epoch_from_year!("2008"),
        batch_ends_at:                 epoch_from_year!("2020"),
        added_to_batch_at:             epoch_from_year!("2011"),
        weight:                        0,
        blocked_companies:             vec![],
        avatar_url:                    "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250".to_owned(),
        salary_expectations:           vec![],
        latest_position:               "".to_owned(),
        languages:                     vec!["English".to_owned()]
      }
    ];

    Talent::index(&mut client, &config.es.index, talents).is_ok()
  }

  fn refresh_index(client: &mut Client) {
    client.refresh()
          .with_indexes(&[&config.es.index])
          .send()
          .unwrap();
  }

  #[test]
  fn test_search() {
    let mut client = make_client();

    if let Err(_) = Talent::reset_index(&mut client, &*config.es.index) {
      let _ = Talent::reset_index(&mut client, &*config.es.index);
    }

    refresh_index(&mut client);

    let _ = populate_index(&mut client);
    let _ = populate_index(&mut client);
    refresh_index(&mut client);

    // no parameters are given
    {
      let results = Talent::search(&mut client, &*config.es.index, &Map::new());
      assert_eq!(vec![4, 5, 2, 1], results.ids());
      assert_eq!(4, results.total);
      assert!(results.talents[0].score.is_none());
      assert!(results.highlights().iter().all(|r| r.is_none()));
    }

    {
      assert!(Talent::delete(&mut client, "1", &*config.es.index).is_ok());
      assert!(Talent::delete(&mut client, "4", &*config.es.index).is_ok());
      refresh_index(&mut client);

      let results = Talent::search(&mut client, &*config.es.index, &Map::new());
      assert_eq!(vec![5, 2], results.ids());

      assert!(populate_index(&mut client));
      refresh_index(&mut client);
    }

    // a non existing index is given
    {
      let mut params = Map::new();
      params.assign("index", Value::String("lololol".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert!(results.is_empty());
    }

    // a date that doesn't match given indexes is given
    {
      let mut params = Map::new();
      params.assign("epoch", Value::String(epoch_from_year!("2040"))).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert!(results.is_empty());
    }

    // a date that match only some talents is given
    {
      let mut params = Map::new();
      params.assign("epoch", Value::String(epoch_from_year!("2006"))).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![2, 1], results.ids());
    }

    // searching for work roles
    {
      let mut params = Map::new();
      params.assign("desired_work_roles[]", Value::String("Fullstack".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4, 5], results.ids());
    }

    // searching for work experience
    {
      let mut params = Map::new();
      params.assign("professional_experience[]", Value::String("8+".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![2], results.ids());
    }

    // searching for work locations
    {
      let mut params = Map::new();
      params.assign("work_locations[]", Value::String("Rome".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![2], results.ids());
    }

    // searching for a language
    {
      let mut params = Map::new();
      params.assign("languages[]", Value::String("English".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4, 5, 2], results.ids());
    }

    // searching for languages
    {
      let mut params = Map::new();
      params.assign("languages[]", Value::String("English".into())).unwrap();
      params.assign("languages[]", Value::String("German".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![2], results.ids());
    }

    // searching for a single keyword
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("HTML5".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![1, 2, 5], results.ids());
    }

    // searching for a single, differently cased and incomplete keyword
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("html".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![1, 2, 5], results.ids());
    }

    // searching for keywords and filters
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("Rust, HTML5 and HTML".into())).unwrap();
      params.assign("work_locations[]", Value::String("Rome".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![2], results.ids());
    }

    // searching for a single word that's supposed to be split
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("reactjs".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4], results.ids());
    }

    // searching for the original dotted string
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("react.js".into())).unwrap();
      params.assign("work_locations[]", Value::String("Berlin".into())).unwrap();
      params.assign("desired_work_roles[]", Value::String("Fullstack".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4], results.ids());
    }

    // searching for a non-matching keyword
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("Criogenesi".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert!(results.is_empty());
    }

    // searching for an empty keyword
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4, 5, 2, 1], results.ids());
    }

    // searching for different parts of a single keyword
    // (Java, JavaScript)
    {
      // JavaScript, Java
      {
        let mut params = Map::new();
        params.assign("keywords", Value::String("Java".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &params);
        assert_eq!(vec![2, 5], results.ids());
      }

      // JavaScript
      {
        let mut params = Map::new();
        params.assign("keywords", Value::String("javascript".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &params);
        assert_eq!(vec![5], results.ids());
      }

      // JavaScript, ClojureScript
      {
        let mut params = Map::new();
        params.assign("keywords", Value::String("script".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &params);
        assert_eq!(vec![4, 5], results.ids());
      }
    }

    // Searching for summary
    {
      {
        let mut params = Map::new();
        params.assign("keywords", Value::String("right now".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &params);
        assert_eq!(vec![4], results.ids());
      }

      {
        let mut params = Map::new();
        params.assign("keywords", Value::String("C++".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &params);
        assert_eq!(vec![4, 5], results.ids());
      }

      {
        let mut params = Map::new();
        params.assign("keywords", Value::String("C#".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &params);
        assert_eq!(vec![5], results.ids());
      }

      {
        let mut params = Map::new();
        params.assign("keywords", Value::String("rust and".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &params);
        assert_eq!(vec![1, 2], results.ids());
      }
    }

    // Searching for headline and summary
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("senior".to_owned())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![2, 4, 1], results.ids());
    }

    // Searching for ideal work roles
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("Devops".to_owned())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4, 5], results.ids());
    }

    // Searching for previous job title
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("database admin".to_owned())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![1, 4], results.ids());
    }

    // highlight
    {
      let mut params = Map::new();
      params.assign("keywords", Value::String("C#".into())).unwrap();

      let results    = Talent::search(&mut client, &*config.es.index, &params).talents;
      let highlights = results.into_iter().map(|r| r.highlight.unwrap()).collect::<Vec<HighlightResult>>();
      assert_eq!(Some(&vec![" C#.".to_owned()]), highlights[0].get("summary"));
    }

    // filtering for given company_id (skip contacted talents)
    {
      let mut params = Map::new();
      params.assign("company_id", Value::String("6".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![2, 1], results.ids());
    }

    // filtering for given bookmarks (ids)
    {
      let mut params = Map::new();
      params.assign("ids[]", Value::U64(2)).unwrap();
      params.assign("ids[]", Value::U64(4)).unwrap();
      params.assign("ids[]", Value::U64(1)).unwrap();
      params.assign("ids[]", Value::U64(3)).unwrap();
      params.assign("ids[]", Value::U64(5)).unwrap();
      params.assign("ids[]", Value::U64(6)).unwrap();
      params.assign("ids[]", Value::U64(7)).unwrap();
      params.assign("ids[]", Value::U64(8)).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4, 5, 2, 1], results.ids());
      assert_eq!(4, results.total);
    }

    // filtering for work_authorization
    {
      let mut params = Map::new();
      params.assign("work_authorization[]", Value::String("no".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4], results.ids());
    }

    // ignoring contacted talents
    {
      let mut params = Map::new();
      params.assign("contacted_talents[]", Value::String("2".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4, 5, 1], results.ids());
    }

    // ignoring blocked companies
    {
      let mut params = Map::new();
      params.assign("company_id", Value::U64(22)).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4, 5, 1], results.ids());
    }

    // embedding position scores
    {
      let scores = vec![
        Score { match_id: "2A-2B-9S".to_owned(),      job_id: 1, talent_id: 1, score: 0.545 },
        Score { match_id: "No4-No16-No21".to_owned(), job_id: 1, talent_id: 2, score: 0.454 },
      ];
      assert!(Score::index(&mut client, &config.es.index, scores).is_ok());

      refresh_index(&mut client);

      let mut params = Map::new();
      params.assign("job_id", Value::U64(1)).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &params);
      assert_eq!(vec![4, 5, 2, 1], results.ids());
      assert!(results.highlights().iter().all(|r| r.is_none()));

      let     scores_ = results.scores();
      let mut scores  = scores_.iter();
      assert!(scores.next().unwrap().is_none());
      assert!(scores.next().unwrap().is_none());
      assert!(scores.next().unwrap().is_some());

      let score = scores.next().unwrap();
      assert_eq!(score.clone().unwrap().score, 0.545);
    }

    // when deleting a talent, the attached score is deleted too
    {
      let search  = ScoreSearchBuilder::new().with_talent_id(1).build();
      let results = Score::search(&mut client, &config.es.index, &search);
      assert_eq!(results.total, 1);

      assert!(Talent::delete(&mut client, "1", &*config.es.index).is_ok());

      refresh_index(&mut client);

      let results = Score::search(&mut client, &config.es.index, &search);
      assert_eq!(results.total, 0);
    }
  }

  #[test]
  fn test_json_decode() {
    let payload = "{
      \"id\":13,
      \"desired_work_roles\":[\"C/C++ Engineer\"],
      \"desired_work_roles_experience\":[\"2..4\"],
      \"work_languages\":[\"C++\"],
      \"professional_experience\":\"8+\",
      \"work_locations\":[\"Berlin\"],
      \"current_location\":\"Berlin\",
      \"work_authorization\":\"yes\",
      \"skills\":[\"Rust\"],
      \"summary\":\"Blabla\",
      \"headline\":\"I see things, I do stuff\",
      \"contacted_company_ids\":[1],
      \"accepted\":true,
      \"batch_starts_at\":\"2016-03-04T12:24:00+01:00\",
      \"batch_ends_at\":\"2016-04-11T12:24:00+02:00\",
      \"added_to_batch_at\":\"2016-03-11T12:24:37+01:00\",
      \"weight\":0,
      \"blocked_companies\":[99],
      \"work_experiences\":[\"Frontend developer\", \"SysAdmin\"],
      \"avatar_url\":\"https://secure.gravatar.com/avatar/47ac43379aa70038a9adc8ec88a1241d?s=250&d=https%3A%2F%2Fsecure.gravatar.com%2Favatar%2Fa0b9ad63fb35d210a218c317e0a6284e%3Fs%3D250\",
      \"salary_expectations\": [{\"minimum\": 40000, \"maximum\": 50000, \"currency\": \"EUR\", \"city\": \"Berlin\"}],
      \"latest_position\":\"Developer\",
      \"languages\":[\"English\"]
    }".to_owned();

    let resource: Result<Talent, _> = serde_json::from_str(&payload);
    assert!(resource.is_ok());
    assert_eq!(resource.unwrap().desired_work_roles, vec!["C/C++ Engineer"]);
  }
}
