use chrono::UTC;
use chrono::datetime::DateTime;

use params::*;

use rs_es::Client;
use rs_es::query::{Filter, Query};
use rs_es::units::JsonVal;
use rs_es::operations::search::{Sort, SortField, Order};

use terms::VectorOfTerms;

pub struct Talent;

#[derive(Debug, RustcDecodable)]
struct TalentsSearchResult {
  id: u32
}

impl Talent {
  pub fn visibility_filters(presented_talents: Vec<i32>) -> Vec<Filter> {
    let now = DateTime::timestamp(&UTC::now());

    let visibility_rules = Filter::build_bool()
                                  .with_must(
                                    vec![
                                      Filter::build_term("accepted", true)
                                             .build(),
                                      Filter::build_range("batch_starts_at")
                                             .with_lt(JsonVal::from(now))
                                             .with_format("epoch_second")
                                             .build(),
                                      Filter::build_range("batch_ends_at")
                                             .with_gte(JsonVal::from(now))
                                             .with_format("epoch_second")
                                             .build()
                                    ])
                                  .build();

    if presented_talents.is_empty() {
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

  pub fn search_filters(params: &Map) -> Query {
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

                                     Talent::visibility_filters(
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

  pub fn sorting_criteria() -> Sort {
    Sort::new(
      vec![
        SortField::new("updated_at", Some(Order::Desc)).build()
      ])
  }

  pub fn search(mut es: Client, params: &Map, indexes: &[&str]) -> Vec<u32> {
    let result = es.search_query()
                   .with_indexes(indexes)
                   .with_query(&Talent::search_filters(params))
                   .with_sort(&Talent::sorting_criteria())
                   .send()
                   .ok()
                   .unwrap();

    result.hits.hits.into_iter()
                    .map(|hit| {
                      let talent: TalentsSearchResult = hit.source().unwrap();
                      talent.id
                    })
                    .collect::<Vec<u32>>()
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_visibility_filters() {
    // TODO
  }

  #[test]
  fn test_search_filters() {
    // TODO
  }

  #[test]
  fn test_search() {
    // TODO
  }
}
