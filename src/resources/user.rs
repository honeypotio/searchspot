use rustc_serialize::json::{self, Json, ToJson};

use super::chrono::UTC;

use super::params::*;

use super::rs_es::Client;
use super::rs_es::query::{Filter, Query};
use super::rs_es::units::JsonVal;
use super::rs_es::operations::search::{Sort, SortField, Order};
use super::rs_es::operations::index::IndexResult;
use super::rs_es::operations::mapping::*;
use super::rs_es::error::EsError;

use searchspot::terms::VectorOfTerms;
use searchspot::resource::*;

#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct Talent {
  pub id:                 u32,
  pub accepted:           bool,
  pub work_roles:         Vec<String>,
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

impl Resource for Talent {
  /// Populate the ElasticSearch index with `self`.
  // I'm having problems with bulk actions. Let's wait for the next iteration.
  fn index(&self, mut es: &mut Client, index: &str) -> Result<IndexResult, EsError> {
    es.index(index, "talent")
      .with_doc(&self)
      .with_id(&*self.id.to_string())
      .send()
  }

  /// Query ElasticSearch on given `indexes` and `params` and return the IDs of
  /// the found talents.
  fn search(mut es: &mut Client, default_index: &str, params: &Map) -> Vec<u32> {
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

  /// Reset the given index. All the data will be destroyed and then the index
  /// will be created again. The map that will be used is hardcoded.
  #[allow(unused_must_use)]
  fn reset_index(mut es: &mut Client, index: &str) -> Result<MappingResult, EsError> {
    let mapping = hashmap! {
      "talent" => hashmap! {
        "id" => hashmap! {
          "type" => "integer",
          "index" => "not_analyzed"
        },

        "work_roles" => hashmap! {
          "type" => "string",
          "index" => "not_analyzed"
        },

        "work_experience" => hashmap! {
          "type" => "string",
          "index" => "not_analyzed"
        },

        "work_locations" => hashmap! {
          "type" => "string",
          "index" => "not_analyzed"
        },

        "work_authorization" => hashmap! {
          "type" => "string",
          "index" => "not_analyzed"
        },

        "company_ids" => hashmap! {
          "type" => "integer",
          "index" => "not_analyzed"
        },

        "accepted" => hashmap! {
          "type" => "boolean",
          "index" => "not_analyzed"
        },

        "batch_starts_at" => hashmap! {
          "type" => "date",
          "format" => "dateOptionalTime",
          "index" => "not_analyzed"
        },

        "batch_ends_at" => hashmap! {
          "type" => "date",
          "format" => "dateOptionalTime",
          "index" => "not_analyzed"
        },

        "added_to_batch_at" => hashmap! {
          "type" => "date",
          "format" => "dateOptionalTime",
          "index" => "not_analyzed"
        },

        "weight" => hashmap! {
          "type" => "integer",
          "index" => "not_analyzed"
        },

        "blocked_companies" => hashmap! {
          "type" => "integer",
          "index" => "not_analyzed"
        }
      }
    };

    es.delete_index(index);

    MappingOperation::new(&mut es, index, &mapping).send()
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
                                             .with_format("dateOptionalTime")
                                             .build(),
                                      Filter::build_range("batch_ends_at")
                                             .with_gte(JsonVal::from(epoch))
                                             .with_format("dateOptionalTime")
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
  /// Considering a single row, the terms inside there are ORred,
  /// while through the rows there is an AND.
  /// I.e.: given ["Fullstack", "DevOps"] as `work_roles`, found talents
  /// will present at least one of these roles), but both `work_roles`
  /// and `work_location`, if provided, must be matched successfully.
  fn search_filters(params: &Map, epoch: &str) -> Query {
    let company_id = i32_vec_from_params!(params, "company_id");

    Query::build_filtered(Filter::build_bool()
                                 .with_must(
                                   vec![
                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_roles", &vec_from_params!(params, "work_roles")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_experience", &vec_from_params!(params, "work_experience")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                      "work_authorization", &vec_from_params!(params, "work_authorization")),

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
#[allow(non_upper_case_globals)]
mod tests {
  extern crate chrono;
  use self::chrono::*;

  extern crate rs_es;
  use self::rs_es::Client;

  extern crate params;
  use self::params::*;

  extern crate searchspot;
  use searchspot::config::*;
  use searchspot::resource::*;

  use resources::user::Talent;

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

    {
      let mut map = Map::new();
      map.assign("work_roles[]", Value::String("Fullstack".to_owned())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![4], results);
    }

    // filtering for given company_id
    {
      let mut map = Map::new();
      map.assign("company_id", Value::String("6".into())).unwrap();

      let results = Talent::search(&mut client, &*config.es.index, &map);
      assert_eq!(vec![2, 1], results);
    }
  }
}
