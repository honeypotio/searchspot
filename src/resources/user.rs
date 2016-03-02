use chrono::UTC;
use chrono::datetime::DateTime;

use params::*;

use rs_es::Client;
use rs_es::query::{Filter, Query};
use rs_es::units::JsonVal;
use rs_es::operations::search::{Sort, SortField, Order};

use terms::VectorOfTerms;

pub struct Talent;

impl Talent {
  /// Return a `Vec<Filter>` with visibility criteria for the talents.
  /// The `epoch` must be given as `I64` (UNIX time in seconds) and is
  /// the range in which batches are searched.
  /// If `presented_talents` is provided, talents who match the IDs
  /// contained there skip the standard visibility criteria.
  ///
  /// Basically, the talents must be accepted into the platform and must be
  /// inside a living batch to match the visibility criteria.
  fn visibility_filters(epoch: i64, presented_talents: Vec<i32>) -> Vec<Filter> {
    let visibility_rules = Filter::build_bool()
                                  .with_must(
                                    vec![
                                      Filter::build_term("accepted", true)
                                             .build(),
                                      Filter::build_range("batch_start_at")
                                             .with_lte(JsonVal::from(epoch))
                                             .with_format("epoch_second")
                                             .build(),
                                      Filter::build_range("batch_end_at")
                                             .with_gte(JsonVal::from(epoch))
                                             .with_format("epoch_second")
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
  fn search_filters(params: &Map, epoch: i64) -> Query {
    let company_id = i32_vec_from_params!(params, "company_id");

    Query::build_filtered(Filter::build_bool()
                                 .with_must(
                                   vec![
                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_roles", &vec_from_params!(params, "work_roles")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_languages", &vec_from_params!(params, "work_languages")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_experience", &vec_from_params!(params, "work_experience")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                       "work_locations", &vec_from_params!(params, "work_locations")),

                                     <Filter as VectorOfTerms<String>>::build_terms(
                                      "work_authorization", &vec_from_params!(params, "work_authorization")),

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

  /// Query ElasticSearch on given `indexes` and `params` and return the IDs of
  /// the found talents.
  pub fn search(mut es: &mut Client, default_indexes: Vec<&str>, params: &Map) -> Vec<u32> {
    let epoch = match params.find(&["epoch"]) {
      Some(&Value::I64(epoch)) => epoch,
      _ => DateTime::timestamp(&UTC::now())
    };

    let indexes: Vec<&str> = match params.find(&["index"]) {
      Some(&Value::String(ref index)) => vec![&index[..]],
      _ => default_indexes
    };

    let result = es.search_query()
                   .with_indexes(&indexes)
                   .with_query(&Talent::search_filters(params, epoch))
                   .with_sort(&Talent::sorting_criteria())
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

  /// Return a `Sort` that makes values be sorted for `updated_at`, descendently.
  fn sorting_criteria() -> Sort {
    Sort::new(
      vec![
        SortField::new("updated_at", Some(Order::Desc)).build()
      ])
  }
}

#[cfg(test)]
mod tests {
  use chrono::UTC;
  use chrono::datetime::DateTime;

  use rs_es::Client;
  use rs_es::query::*;
  use rs_es::operations::bulk::Action;
  use rs_es::units::Duration as ESDuration;
  use rs_es::units::DurationUnit;

  use params::*;

  use resources::user::Talent;
  use config::*;
  use rustc_serialize::json::*;

  #[derive(RustcEncodable, Debug)]
  pub struct TestUser {
    pub id:              u32,
    pub accepted:        bool,
    pub batch_start_at:  i64,
    pub batch_end_at:    i64,
    pub updated_at:      i64,
    pub work_roles:      Array,
    pub company_ids:     Array
  }

  pub fn make_client() -> Client {
    let config = Config::from_file("examples/tests.toml".to_owned());
    Client::new(&*config.es.host, config.es.port)
  }

  pub fn populate_es(mut client: &mut Client) {
    let users = vec![
      TestUser {
        id:              1,
        accepted:        true,
        batch_start_at:  1141141876, // 2006
        batch_end_at:    4580812259, // 2099
        updated_at:      DateTime::timestamp(&UTC::now()),
        work_roles:      vec![],
        company_ids:     vec![]
      },

      TestUser {
        id:              2,
        accepted:        false,
        batch_start_at:  1141141876, // 2006
        batch_end_at:    4580812259, // 2099
        updated_at:      DateTime::timestamp(&UTC::now()) + 10,
        work_roles:      vec![],
        company_ids:     vec![]
      },

      TestUser {
        id:              3,
        accepted:        true,
        batch_start_at:  1141141876, // 2006
        batch_end_at:    4580812259, // 2099
        updated_at:      DateTime::timestamp(&UTC::now()) + 20,
        work_roles:      vec!["Fullstack".to_json(), "DevOps".to_json()],
        company_ids:     vec![6.to_json()]
      }];

    for user in users {
      client.index("sample_index", "test_user")
             .with_doc(&user)
             .send()
             .unwrap();
    }
  }

  pub fn clean_es(mut client: &mut Client) {
    let mut scan = match client.search_query()
                               .with_indexes(&["sample_index"])
                               .with_query(&Query::build_match_all().build())
                               .scan(ESDuration::new(1, DurationUnit::Minute)) {
                                   Ok(scan) => scan,
                                   Err(_)   => return
                               };

    loop {
      let     page = scan.scroll(&mut client).unwrap();
      let mut hits = page.hits.hits;

      if hits.len() == 0 {
        break;
      }

      let actions: Vec<Action> = hits.drain(..)
                                .map(|hit| {
                                    Action::delete(hit.id)
                                        .with_index("sample_index")
                                        .with_doc_type(hit.doc_type)
                                })
                                .collect();
      client.bulk(&actions).send().unwrap();
    }

    scan.close(&mut client).unwrap();
  }

  #[test]
  fn test_search() {
    let mut client = make_client();
    clean_es(&mut client);
    populate_es(&mut client);

    // no parameters are given
    {
      let results = Talent::search(&mut client, vec!["sample_index"], &Map::new());
      assert_eq!(vec![3, 1], results);
    }

    // a non existing index is given
    {
      let mut map = Map::new();
      map.assign("index", Value::String("lololol".to_owned())).unwrap();

      let results = Talent::search(&mut client, vec!["sample_index"], &map);
      assert!(results.is_empty());
    }

    // a date that doesn't match given indexes is given
    {
      let mut map = Map::new();
      map.assign("epoch", Value::I64(1141141870)).unwrap();

      let results = Talent::search(&mut client, vec!["sample_index"], &map);
      assert!(results.is_empty());
    }

    // a date that doesn't match given indexes is given
    {
      let mut map = Map::new();
      map.assign("epoch", Value::I64(1141141870)).unwrap();

      let results = Talent::search(&mut client, vec!["sample_index"], &map);
      assert!(results.is_empty());
    }

    // TODO
    // filtering for valid work roles
    /*{
      let mut map = Map::new();
      map.assign("work_roles[]", Value::String("Fullstack".to_owned())).unwrap();

      let results = Talent::search(&mut client, vec!["sample_index"], &map);
      assert_eq!(vec![3], results);
    }*/

    // filtering for valid work roles
    {
      let mut map = Map::new();
      map.assign("company_id", Value::String("6".into())).unwrap();

      let results = Talent::search(&mut client, vec!["sample_index"], &map);
      assert_eq!(vec![1], results);
    }
  }
}
