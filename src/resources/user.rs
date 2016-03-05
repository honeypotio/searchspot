use chrono::UTC;
use rustc_serialize::json::{self, Json, ToJson};

use params::*;

use rs_es::Client;
use rs_es::query::{Filter, Query};
use rs_es::units::{JsonVal, DurationUnit};
use rs_es::units::Duration as ESDuration;
use rs_es::operations::search::{Sort, SortField, Order};
use rs_es::operations::index::IndexResult;
use rs_es::operations::bulk::{Action, BulkResult};
use rs_es::error::EsError;

extern crate searchspot;
use searchspot::terms::VectorOfTerms;

#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct Talent {
  pub id:                 u32,
  pub accepted:           bool,
  pub work_roles:         Vec<String>,
  pub work_languages:     Vec<String>,
  pub work_experience:    String,
  pub work_locations:     Vec<String>,
  pub work_authorization: String,
  pub company_ids:        Vec<u32>,
  pub batch_starts_at:    String,
  pub batch_ends_at:      String,
  pub added_to_batch_at:  String,
  pub weight:             i32,
  pub blocked_companies:  Vec<u32>
}

impl ToJson for Talent {
  fn to_json(&self) -> Json {
    json::encode(&self).unwrap()
                       .to_json()
  }
}

impl Talent {
  /// Populate the ElasticSearch index with `self`.
  pub fn index(&self, mut es: &mut Client, index: &str) -> Result<IndexResult, EsError> {
    es.index(index, "talent")
      .with_doc(&self)
      .with_id(&*self.id.to_string())
      .send()
  }

  /// Populate the ElasticSearch index with `Vec<Talent>`.
  pub fn index_many(talents: Vec<Talent>, mut es: &mut Client, index: &str) -> Result<BulkResult, EsError> {
    let actions: Vec<Action> = talents.into_iter().map(|talent| {
      let id   = &*talent.id.to_string();
      let json = json::encode(&talent).unwrap()
                                      .to_json();

      Action::index(json).with_id(id)
                         .with_index(index)
    }).collect();

    es.bulk(&actions).send()
  }

  #[allow(dead_code)]
  pub fn delete_all(mut es: &mut Client, index: &str) {
    let mut scan = match es.search_query()
                           .with_indexes(&[&index])
                           .with_query(&Query::build_match_all().build())
                           .scan(ESDuration::new(1, DurationUnit::Minute)) {
                              Ok(scan) => scan,
                              Err(_)   => return
                           };

    loop {
      let     page = scan.scroll(&mut es).unwrap();
      let mut hits = page.hits.hits;

      if hits.len() == 0 {
        break;
      }

      let actions = hits.drain(..)
                        .map(|hit| {
                           Action::delete(hit.id).with_index(&*index)
                                                 .with_doc_type(hit.doc_type)
                        })
                        .collect::<Vec<Action>>();
      es.bulk(&actions)
        .send()
        .unwrap();
    }

    scan.close(&mut es)
        .unwrap();
  }

  /// Query ElasticSearch on given `indexes` and `params` and return the IDs of
  /// the found talents.
  pub fn search(mut es: &mut Client, default_index: &str, params: &Map) -> Vec<u32> {
    let now   = UTC::now().to_rfc3339();
    let epoch = match params.find(&["epoch"]) {
      Some(epoch) => String::from_value(&epoch).unwrap_or(now),
      _           => now
    };

    let index: Vec<&str> = match params.find(&["index"]) {
      Some(&Value::String(ref index)) => vec![&index[..]],
      _ => vec![default_index]
    };

    let result = es.search_query()
                   .with_indexes(&*index)
                   .with_query(&Talent::search_filters(params, &*epoch))
                   .with_sort(&Talent::sorting_criteria())
                   .with_size(1000) // TODO
                   .send();

    match result {
      Ok(result) => {
        let mut results = result.hits.hits.into_iter()
                                          .map(|hit| hit.source.unwrap()["id"]
                                                               .as_u64()
                                                               .unwrap() as u32)
                                          .collect::<Vec<u32>>();
        results.dedup();
        results
      },
      Err(err) => {
        println!("{:?}", err);
        vec![]
      }
    }
  }

  /// Return a `Vec<Filter>` with visibility criteria for the talents.
  /// The `epoch` must be given as `I64` (UNIX time in seconds) and is
  /// the range in which batches are searched.
  /// If `presented_talents` is provided, talents who match the IDs
  /// contained there skip the standard visibility criteria.
  ///
  /// Basically, the talents must be accepted into the platform and must be
  /// inside a living batch to match the visibility criteria.
  fn visibility_filters(epoch: &str, presented_talents: Vec<i32>) -> Vec<Filter> {
    let visibility_rules = Filter::build_bool()
                                  .with_must(
                                    vec![
                                      Filter::build_term("accepted", true)
                                             .build(),
                                      Filter::build_range("batch_starts_at")
                                             .with_lte(JsonVal::from(epoch))
                                             .with_format("strict_date_optional_time")
                                             .build(),
                                      Filter::build_range("batch_ends_at")
                                             .with_gte(JsonVal::from(epoch))
                                             .with_format("strict_date_optional_time")
                                             .build()
                                    ])
                                  .build();

    if presented_talents.len() > 0 { // preferred over !_.is_empty()
      let presented_talents_filters = Filter::build_bool()
                                             .with_must(
                                               vec![
                                                 <Filter as VectorOfTerms<i32>>::build_terms(
                                                   "ids", &presented_talents)
                                               ].into_iter()
                                                .flat_map(|x| x)
                                                .collect::<Vec<Filter>>())
                                             .build();
      vec![
        Filter::build_bool()
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
  /// `VectorOfTerms` are ORred, while `Filter`s are ANDed.
  /// I.e.: given ["Fullstack", "DevOps"] as `work_roles`, found talents
  /// will present at least one of these roles), but both `work_roles`
  /// and `work_languages`, if provided, must not be empty.
  fn search_filters(params: &Map, epoch: &str) -> Query {
    let company_id = i32_vec_from_params!(params, "company_id");

    Query::build_filtered(Filter::build_bool()
                                 .with_must(
                                   vec![
                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_roles", &vec_from_params!(params, "work_roles")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_languages", &vec_from_params!(params, "work_languages")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_experience", &string_vec_from_params!(params, "work_experience")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                      "work_authorization", &string_vec_from_params!(params, "work_authorization")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_locations", &vec_from_params!(params, "work_locations")),

                                     Talent::visibility_filters(epoch,
                                       i32_vec_from_params!(params, "presented_talents"))
                                   ].into_iter()
                                    .flat_map(|x| x)
                                    .collect::<Vec<Filter>>())
                                 .with_must_not(
                                   vec![
                                     <Filter as VectorOfTerms<i32>>::build_terms(
                                       "company_ids", &company_id),

                                     <Filter as VectorOfTerms<i32>>::build_terms(
                                       "blocked_companies", &company_id)
                                   ].into_iter()
                                    .flat_map(|x| x)
                                    .collect::<Vec<Filter>>())
                                 .build())
          .build()
  }

  /// Return a `Sort` that makes values be sorted for given fields, descendently.
  fn sorting_criteria() -> Sort {
    Sort::new(
      vec![
        SortField::new("batch_starts_at",   Some(Order::Desc)).build(),
        SortField::new("weight",            Some(Order::Desc)).build(),
        SortField::new("added_to_batch_at", Some(Order::Desc)).build()
      ])
  }
}

#[cfg(test)]
mod tests {
  use chrono::*;

  use rs_es::Client;

  use params::*;

  extern crate searchspot;
  use searchspot::config::*;

  use resources::user::Talent;

  use std::time::Duration as TimeDuration;
  use std::thread::sleep;

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

  pub fn populate_es(mut client: &mut Client) {
    Talent::index_many(vec![
      Talent {
        id:                 1,
        accepted:           true,
        work_roles:         vec![],
        work_languages:     vec![],
        work_experience:    "1..2".to_owned(),
        work_locations:     vec!["Berlin".to_owned()],
        work_authorization: "yes".to_owned(),
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
        work_languages:     vec![],
        work_experience:    "1..2".to_owned(),
        work_locations:     vec!["Berlin".to_owned()],
        work_authorization: "yes".to_owned(),
        company_ids:        vec![],
        batch_starts_at:    epoch_from_year!("2006"),
        batch_ends_at:      epoch_from_year!("2020"),
        added_to_batch_at:  epoch_from_year!("2006"),
        weight:             6,
        blocked_companies:  vec![]
      },

      Talent {
        id:                 3,
        accepted:           false,
        work_roles:         vec![],
        work_languages:     vec![],
        work_experience:    "1..2".to_owned(),
        work_locations:     vec!["Berlin".to_owned()],
        work_authorization: "yes".to_owned(),
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
        work_languages:     vec![],
        work_experience:    "1..2".to_owned(),
        work_locations:     vec!["Berlin".to_owned()],
        work_authorization:  "yes".to_owned(),
        company_ids:        vec![6],
        batch_starts_at:    epoch_from_year!("2008"),
        batch_ends_at:      epoch_from_year!("2020"),
        added_to_batch_at:  epoch_from_year!("2011"),
        weight:             0,
        blocked_companies:  vec![]
      }
    ], &mut client, &config.es.index);

    sleep(TimeDuration::from_millis(5000));
  }

  #[test]
  fn test_search() {
    let mut client = make_client();
    Talent::delete_all(&mut client, &config.es.index);
    populate_es(&mut client);

    // no parameters are given
    {
      let results = Talent::search(&mut client, &*config.es.index, &Map::new());
      assert_eq!(vec![4, 2, 1], results);
    }

    // a non existing index is given
    {
      let mut map = Map::new();
      map.assign("index", Value::String("lololol".to_owned())).unwrap();

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

    // TODO: map field type properly (see benashford/rs-es#11)
    // // filtering for valid work roles
    // // {
    // //   let mut map = Map::new();
    // //   map.assign("work_roles[]", Value::String("Fullstack".to_owned())).unwrap();
    // //
    // //   let results = Talent::search(&mut client, &*config.es.index, &map);
    // //   assert_eq!(vec![4], results);
    // // }

    // filtering for given company_id
    {
      let mut map = Map::new();
      map.assign("company_id", Value::String("6".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![2, 1], results);
    }
  }
}
