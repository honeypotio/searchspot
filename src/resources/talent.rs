use super::chrono::UTC;

use super::params::*;
use super::serde_json::Value as JsonValue;

use super::rs_es::Client;
use super::rs_es::query::Query;
use super::rs_es::operations::search::{Sort, SortField, Order};
use super::rs_es::operations::index::IndexResult;
use super::rs_es::operations::mapping::*;
use super::rs_es::query::full_text::MatchQueryType;
use super::rs_es::error::EsError;
use super::rs_es::operations::search::highlight::*;

use searchspot::terms::VectorOfTerms;
use searchspot::matches::VectorOfMatches;
use searchspot::resource::*;

/// The type that we use in ElasticSearch for defining a Talent.
const ES_TYPE: &'static str = "talent";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResults {
  pub results: Vec<SearchResult>
}

impl SearchResults {
  pub fn new(results: Vec<SearchResult>) -> SearchResults {
    SearchResults { results: results }
  }

  #[allow(dead_code)]
  pub fn ids(&self) -> Vec<u32> {
    self.results.iter().map(|r| r.talent.id).collect()
  }

  #[allow(dead_code)]
  pub fn highlights(&self) -> Vec<Option<HighlightResult>> {
    self.results.iter().map(|r| r.highlight.clone()).collect()
  }

  #[allow(dead_code)]
  pub fn is_empty(&self) -> bool {
    self.results.is_empty()
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
  pub talent:    FoundTalent,
  pub highlight: Option<HighlightResult>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FoundTalent {
  pub id: u32
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Talent {
  pub id:                 u32,
  pub accepted:           bool,
  pub work_roles:         Vec<String>,
  pub work_experience:    String,
  pub work_locations:     Vec<String>,
  pub work_authorization: String,
  pub skills:             Vec<String>,
  pub summary:            String,
  pub headline:           String,
  pub job_titles:         Vec<String>,
  pub company_ids:        Vec<u32>,
  pub batch_starts_at:    String,
  pub batch_ends_at:      String,
  pub added_to_batch_at:  String,
  pub weight:             i32,
  pub blocked_companies:  Vec<u32>
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
  pub fn visibility_filters(epoch: &str, presented_talents: Vec<i32>) -> Vec<Query> {
    let visibility_rules = Query::build_bool()
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
  /// I.e.: given ["Fullstack", "DevOps"] as `work_roles`, found talents
  /// will present at least one of these roles), but both `work_roles`
  /// and `work_location`, if provided, must be matched successfully.
  pub fn search_filters(params: &Map, epoch: &str) -> Query {
    let company_id = i32_vec_from_params!(params, "company_id");

    Query::build_bool()
          .with_must(
             vec![
                match Talent::full_text_search(params) {
                  Some(keywords) => vec![keywords],
                  None           => vec![]
                },

               <Query as VectorOfMatches<String>>::build_match(
                 "work_roles", &vec_from_params!(params, "work_roles")),

               <Query as VectorOfTerms<String>>::build_terms(
                 "work_experience", &vec_from_params!(params, "work_experience")),

               <Query as VectorOfTerms<String>>::build_terms(
                 "work_authorization", &vec_from_params!(params, "work_authorization")),

               <Query as VectorOfTerms<String>>::build_terms(
                 "work_locations", &vec_from_params!(params, "work_locations")),

               <Query as VectorOfTerms<i32>>::build_terms(
                 "id", &vec_from_params!(params, "ids")),

               Talent::visibility_filters(epoch,
                 i32_vec_from_params!(params, "presented_talents"))
               ].into_iter()
                .flat_map(|x| x)
                .collect::<Vec<Query>>())
                .with_must_not(
                   vec![
                     <Query as VectorOfTerms<i32>>::build_terms(
                       "company_ids", &company_id),

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
      Some(keywords) => match keywords {
        &Value::String(ref keywords) => match keywords.is_empty() {
          true  => None,
          false => Some(
              Query::build_multi_match(
                  vec![
                    "skills".to_owned(),
                    "summary".to_owned(),
                    "headline".to_owned(),
                    "work_roles".to_owned(),
                    "job_titles".to_owned()
                  ], keywords.to_owned())
             .with_type(MatchQueryType::CrossFields)
             .with_tie_breaker(0.0)
             .build())
        },
        _ => None
      },
      None => None
    }
  }

  /// Return a `Sort` that makes values be sorted for given fields, descendently.
  pub fn sorting_criteria() -> Sort {
    Sort::new(
      vec![
        SortField::new("batch_starts_at",   Some(Order::Desc)).build(),
        SortField::new("weight",            Some(Order::Desc)).build(),
        SortField::new("added_to_batch_at", Some(Order::Desc)).build()
      ])
  }
}

impl Resource for Talent {
  type Results = SearchResults;

  /// Populate the ElasticSearch index with `self`.
  // I'm having problems with bulk actions. Let's wait for the next iteration.
  fn index(&self, mut es: &mut Client, index: &str) -> Result<IndexResult, EsError> {
    es.index(index, ES_TYPE)
      .with_doc(&self)
      .with_id(&*self.id.to_string())
      .send()
  }

  /// Query ElasticSearch on given `indexes` and `params` and return the IDs of
  /// the found talents.
  fn search(mut es: &mut Client, default_index: &str, params: &Map) -> Self::Results {
    let now   = UTC::now().to_rfc3339();
    let epoch = match params.find(&["epoch"]) {
      Some(epoch) => String::from_value(&epoch).unwrap_or(now),
      _           => now
    };

    let index: Vec<&str> = match params.find(&["index"]) {
      Some(&Value::String(ref index)) => vec![&index[..]],
      _ => vec![default_index]
    };

    let keywords_present = match params.get("keywords") {
      Some(keywords) => match keywords {
        &Value::String(ref keywords) => !keywords.is_empty(),
        _                            => false
      },
      None => false
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
      highlight.add_setting("work_roles".to_owned(), settings.clone());
      highlight.add_setting("job_titles".to_owned(), settings);

      es.search_query()
        .with_indexes(&*index)
        .with_query(&Talent::search_filters(params, &*epoch))
        .with_highlight(&highlight)
        .with_size(1000) // TODO
        .send::<Talent>()
    }
    else {
      es.search_query()
        .with_indexes(&*index)
        .with_query(&Talent::search_filters(params, &*epoch))
        .with_sort(&Talent::sorting_criteria())
        .with_size(1000) // TODO
        .send::<Talent>()
    };

    match result {
      Ok(result) => {
        SearchResults::new(result.hits.hits.into_iter()
                                           .filter(|hit| {
                                              match hit.score {
                                                Some(score) => score > 0.55,
                                                None        => true
                                              }
                                            })
                                           .map(|hit| {
                                             SearchResult {
                                               talent: FoundTalent { id: hit.source.unwrap().id },
                                               highlight: hit.highlight
                                             }
                                           })
                                           .collect::<Vec<SearchResult>>())
      },
      Err(err) => {
        println!("{:?}", err);
        SearchResults::new(vec![])
      }
    }
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

        "work_roles" => hashmap! {
          "type"            => "string",
          "analyzer"        => "trigrams",
          "search_analyzer" => "words"
        },

        "work_experience" => hashmap! {
          "type"  => "string",
          "index" => "not_analyzed"
        },

        "work_locations" => hashmap! {
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

        "job_titles" => hashmap! {
          "type"            => "string",
          "analyzer"        => "trigrams",
          "search_analyzer" => "words"
        },

        "company_ids" => hashmap! {
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
        }
      }
    };

    let settings = Settings {
      number_of_shards: 1,

      analysis: Analysis {
        filter: btreemap! {
          "trigrams_filter".to_owned() => JsonValue::Object(btreemap! {
            "type".to_owned()     => JsonValue::String("ngram".into()),
            "min_gram".to_owned() => JsonValue::U64(2),
            "max_gram".to_owned() => JsonValue::U64(20)
          }),

          "words_splitter".to_owned() => JsonValue::Object(btreemap! {
            "type".to_owned()              => JsonValue::String("word_delimiter".into()),
            "preserve_original".to_owned() => JsonValue::Bool(true),
            "catenate_all".to_owned()      => JsonValue::Bool(true)
          }),

          "english_words_filter".to_owned() => JsonValue::Object(btreemap! {
            "type".to_owned()      => JsonValue::String("stop".into()),
            "stopwords".to_owned() => JsonValue::String("_english_".into())
          }),

          "tech_words_filter".to_owned() => JsonValue::Object(btreemap! {
            "type".to_owned()      => JsonValue::String("stop".into()),
            "stopwords".to_owned() => JsonValue::Array(
                                        vec![
                                          JsonValue::String("js".into())
                                        ])
          })
        },
        analyzer: btreemap! {
          "trigrams".to_owned() => JsonValue::Object(btreemap! { // index time
            "type".to_owned()      => JsonValue::String("custom".into()),
            "tokenizer".to_owned() => JsonValue::String("whitespace".into()),
            "filter".to_owned()    => JsonValue::Array(
                                        vec![
                                          JsonValue::String("lowercase".into()),
                                          JsonValue::String("words_splitter".into()),
                                          JsonValue::String("trigrams_filter".into()),
                                          JsonValue::String("english_words_filter".into()),
                                          JsonValue::String("tech_words_filter".into())
                                        ])
          }),

          "words".to_owned() => JsonValue::Object(btreemap! { // query time
            "type".to_owned()      => JsonValue::String("custom".into()),
            "tokenizer".to_owned() => JsonValue::String("keyword".into()),
            "filter".to_owned()    => JsonValue::Array(
                                        vec![
                                          JsonValue::String("lowercase".into()),
                                          JsonValue::String("words_splitter".into()),
                                          JsonValue::String("english_words_filter".into()),
                                          JsonValue::String("tech_words_filter".into())
                                        ])
          })
        }
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
#[allow(non_upper_case_globals)]
mod tests {
  extern crate serde_json;

  extern crate chrono;
  use self::chrono::*;

  extern crate rs_es;
  use self::rs_es::Client;
  use self::rs_es::operations::search::highlight::HighlightResult;

  extern crate params;
  use self::params::*;

  extern crate searchspot;
  use searchspot::config::*;
  use searchspot::resource::*;

  use resources::Talent;

  const CONFIG_FILE: &'static str = "examples/tests.toml";

  lazy_static! {
    static ref config: Config = Config::from_file(CONFIG_FILE.to_owned());
  }

  pub fn make_client() -> Client {
    Client::new(&*config.es.host, config.es.port)
  }

  macro_rules! epoch_from_year {
    ($year:expr) => {
      UTC.datetime_from_str(&format!("{}-01-01 12:00:00", $year),
        "%Y-%m-%d %H:%M:%S").unwrap().to_rfc3339()
    }
  }

  pub fn populate_index(mut client: &mut Client) -> bool {
    vec![
      Talent {
        id:                 1,
        accepted:           true,
        work_roles:         vec![],
        work_experience:    "1..2".to_owned(),
        work_locations:     vec!["Berlin".to_owned()],
        work_authorization: "yes".to_owned(),
        skills:             vec!["Rust".to_owned(), "HTML5".to_owned(), "HTML".to_owned()],
        summary:            "I'm a senior Rust developer and sometimes I do also HTML.".to_owned(),
        headline:           "Backend developer with Rust experience".to_owned(),
        job_titles:         vec!["Database Administrator".to_owned()],
        company_ids:        vec![],
        batch_starts_at:    epoch_from_year!("2006"),
        batch_ends_at:      epoch_from_year!("2020"),
        added_to_batch_at:  epoch_from_year!("2006"),
        weight:             -5,
        blocked_companies:  vec![]
      },

      Talent {
        id:                 2,
        accepted:           true,
        work_roles:         vec![],
        work_experience:    "8+".to_owned(),
        work_locations:     vec!["Rome".to_owned(),"Berlin".to_owned()],
        work_authorization: "yes".to_owned(),
        skills:             vec!["Rust".to_owned(), "HTML5".to_owned(), "Java".to_owned()],
        summary:            "I'm a java dev with some tricks up my sleeves".to_owned(),
        headline:           "Senior Java engineer".to_owned(),
        job_titles:         vec![],
        company_ids:        vec![],
        batch_starts_at:    epoch_from_year!("2006"),
        batch_ends_at:      epoch_from_year!("2020"),
        added_to_batch_at:  epoch_from_year!("2006"),
        weight:             6,
        blocked_companies:  vec![22]
      },

      Talent {
        id:                 3,
        accepted:           false,
        work_roles:         vec![],
        work_experience:    "1..2".to_owned(),
        work_locations:     vec!["Berlin".to_owned()],
        work_authorization: "yes".to_owned(),
        skills:             vec![],
        summary:            "".to_owned(),
        headline:           "".to_owned(),
        job_titles:         vec![],
        company_ids:        vec![],
        batch_starts_at:    epoch_from_year!("2007"),
        batch_ends_at:      epoch_from_year!("2020"),
        added_to_batch_at:  epoch_from_year!("2011"),
        weight:             6,
        blocked_companies:  vec![]
      },

      Talent {
        id:                 4,
        accepted:           true,
        work_roles:         vec!["Fullstack".to_owned(), "DevOps".to_owned()],
        work_experience:    "1..2".to_owned(),
        work_locations:     vec!["Berlin".to_owned()],
        work_authorization: "no".to_owned(),
        skills:             vec!["ClojureScript".to_owned(), "C++".to_owned(), "React.js".to_owned()],
        summary:            "ClojureScript right now, previously C++".to_owned(),
        headline:           "Senior fullstack developer with sysadmin skills".to_owned(),
        job_titles:         vec!["Backend Engineer".to_owned(), "Database Administrator".to_owned()],
        company_ids:        vec![6],
        batch_starts_at:    epoch_from_year!("2008"),
        batch_ends_at:      epoch_from_year!("2020"),
        added_to_batch_at:  epoch_from_year!("2011"),
        weight:             0,
        blocked_companies:  vec![]
      },

      Talent {
        id:                 5,
        accepted:           true,
        work_roles:         vec!["Fullstack".to_owned(), "DevOps".to_owned()],
        work_experience:    "1..2".to_owned(),
        work_locations:     vec!["Berlin".to_owned()],
        work_authorization: "yes".to_owned(),
        skills:             vec!["JavaScript".to_owned(), "C++".to_owned(), "Ember.js".to_owned()],
        summary:            "C++ and frontend dev. HTML, C++, JavaScript and C#. Did I say C++?".to_owned(),
        headline:           "Amazing C developer".to_owned(),
        job_titles:         vec![],
        company_ids:        vec![6],
        batch_starts_at:    epoch_from_year!("2008"),
        batch_ends_at:      epoch_from_year!("2020"),
        added_to_batch_at:  epoch_from_year!("2011"),
        weight:             0,
        blocked_companies:  vec![]
      }
    ].iter()
     .map(|talent| talent.index(&mut client, &config.es.index)
                         .is_ok())
     .collect::<Vec<bool>>()
     .into_iter()
     .all(|result| result)
  }

  fn refresh_index(mut client: &mut Client) {
    client.refresh()
          .with_indexes(&[&config.es.index])
          .send()
          .unwrap();
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
      let results = Talent::search(&mut client, &*config.es.index, &Map::new());
      assert_eq!(vec![4, 5, 2, 1], results.ids());
      assert!(results.highlights().iter().all(|r| r.is_none()));
    }

    // a non existing index is given
    {
      let mut map = Map::new();
      map.assign("index", Value::String("lololol".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert!(results.is_empty());
    }

    // a date that doesn't match given indexes is given
    {
      let mut map = Map::new();
      map.assign("epoch", Value::String(epoch_from_year!("2040"))).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert!(results.is_empty());
    }

    // searching for work roles
    {
      let mut map = Map::new();
      map.assign("work_roles[]", Value::String("Fullstack".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4, 5], results.ids());
    }

    // searching for work experience
    {
      let mut map = Map::new();
      map.assign("work_experience[]", Value::String("8+".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![2], results.ids());
    }

    // searching for work locations
    {
      let mut map = Map::new();
      map.assign("work_locations[]", Value::String("Rome".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![2], results.ids());
    }

    // searching for a single keyword
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("HTML5".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![1, 2, 5], results.ids());
    }

    // searching for a single, differently cased and incomplete keyword
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("html".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![1, 2, 5], results.ids());
    }

    // searching for keywords and filters
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("Rust, HTML5 and HTML".into())).unwrap();
      map.assign("work_locations[]", Value::String("Rome".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![2], results.ids());
    }

    // searching for a single word that's supposed to be split
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("reactjs".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4], results.ids());
    }

    // searching for the original dotted string
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("react.js".into())).unwrap();
      map.assign("work_locations[]", Value::String("Berlin".into())).unwrap();
      map.assign("work_roles[]", Value::String("Fullstack".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4], results.ids());
    }

    // searching for a non-matching keyword
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("Criogenesi".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert!(results.is_empty());
    }

    // searching for an empty keyword
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4, 5, 2, 1], results.ids());
    }

    // searching for different parts of a single keyword
    // (Java, JavaScript)
    {
      // JavaScript, Java
      {
        let mut map = Map::new();
        map.assign("keywords", Value::String("Java".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &map);
        assert_eq!(vec![2, 5], results.ids());
      }

      // JavaScript
      {
        let mut map = Map::new();
        map.assign("keywords", Value::String("javascript".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &map);
        assert_eq!(vec![5], results.ids());
      }

      // JavaScript, ClojureScript
      {
        let mut map = Map::new();
        map.assign("keywords", Value::String("script".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &map);
        assert_eq!(vec![4, 5], results.ids());
      }
    }

    // Searching for summary
    {
      {
        let mut map = Map::new();
        map.assign("keywords", Value::String("right now".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &map);
        assert_eq!(vec![4], results.ids());
      }

      {
        let mut map = Map::new();
        map.assign("keywords", Value::String("C++".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &map);
        assert_eq!(vec![4, 5], results.ids());
      }

      {
        let mut map = Map::new();
        map.assign("keywords", Value::String("C#".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &map);
        assert_eq!(vec![5], results.ids());
      }

      {
        let mut map = Map::new();
        map.assign("keywords", Value::String("rust and".into())).unwrap();

        let results = Talent::search(&mut client, &*config.es.index, &map);
        assert_eq!(vec![1, 2], results.ids());
      }
    }

    // Searching for headline and summary
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("senior".to_owned())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![2, 4, 1], results.ids());
    }

    // Searching for ideal work roles
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("Devops".to_owned())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4, 5], results.ids());
    }

    // Searching for previous job title
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("database admin".to_owned())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![1, 4], results.ids());
    }

    // highlight
    {
      let mut map = Map::new();
      map.assign("keywords", Value::String("C#".into())).unwrap();

      let results    = Talent::search(&mut client, &*config.es.index, &map).results;
      let highlights = results.into_iter().map(|r| r.highlight.unwrap()).collect::<Vec<HighlightResult>>();
      assert_eq!(Some(&vec![" C#.".to_owned()]), highlights[0].get("summary"));
    }

    // filtering for given company_id
    {
      let mut map = Map::new();
      map.assign("company_id", Value::String("6".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![2, 1], results.ids());
    }

    // filtering for given bookmarks (ids)
    {
      let mut map = Map::new();
      map.assign("ids[]", Value::U64(2)).unwrap();
      map.assign("ids[]", Value::U64(4)).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4, 2], results.ids());
    }

    // filtering for work_authorization
    {
      let mut map = Map::new();
      map.assign("work_authorization[]", Value::String("no".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4], results.ids());
    }

    // ignoring contacted talents
    {
      let mut map = Map::new();
      map.assign("contacted_talents[]", Value::String("2".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4, 5, 1], results.ids());
    }

    // ignoring blocked companies
    {
      let mut map = Map::new();
      map.assign("company_id", Value::U64(22)).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4, 5, 1], results.ids());
    }
  }

  #[test]
  fn test_json_decode() {
    let payload = "{
      \"id\":13,
      \"work_roles\":[\"C/C++ Engineer\"],
      \"work_languages\":[],
      \"work_experience\":\"8+\",
      \"work_locations\":[\"Berlin\"],
      \"work_authorization\":\"yes\",
      \"skills\":[\"Rust\"],
      \"summary\":\"\",
      \"headline\":\"\",
      \"job_titles\":[],
      \"company_ids\":[],
      \"accepted\":true,
      \"batch_starts_at\":\"2016-03-04T12:24:00+01:00\",
      \"batch_ends_at\":\"2016-04-11T12:24:00+02:00\",
      \"added_to_batch_at\":\"2016-03-11T12:24:37+01:00\",
      \"weight\":0,
      \"blocked_companies\":[]
    }".to_owned();

    let resource: Result<Talent, _> = serde_json::from_str(&payload);
    assert!(resource.is_ok());
    assert_eq!(resource.unwrap().work_roles, vec!["C/C++ Engineer"]);
  }
}
