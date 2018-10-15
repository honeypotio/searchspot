use chrono::prelude::*;

use params::{FromValue, Map, Value};

use rs_es::error::EsError;
use rs_es::operations::bulk::{Action, BulkResult};
use rs_es::operations::delete::DeleteResult;
use rs_es::operations::mapping::{Analysis, MappingOperation, MappingResult, Settings};
use rs_es::operations::search::highlight::{Encoders, Highlight, HighlightResult, Setting,
                                           SettingTypes, TermVector};
use rs_es::operations::search::{Order, SearchHitsHitsResult, Sort, SortField};
use rs_es::query::Query;
use rs_es::Client;

use resource::Resource;
use terms::VectorOfTerms;

use std::collections::{HashSet, HashMap};

/// The type that we use in ElasticSearch for defining a `Talent`.
const ES_TYPE: &'static str = "talent";

/// A collection of `SearchResult`s.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SearchResults {
    pub total: u64,
    pub talents: Vec<SearchResult>,
    pub raw_es_query: Option<String>,
}

/// A single search result returned by ElasticSearch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
    pub talent: FoundTalent,
    pub highlight: Option<HighlightResult>,
}

/// Convert an ElasticSearch result into a `SearchResult`.
impl From<SearchHitsHitsResult<Talent>> for SearchResult {
    fn from(result: SearchHitsHitsResult<Talent>) -> SearchResult {
        SearchResult {
            talent: result.source.unwrap().into(),
            highlight: result.highlight,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SalaryExpectations {
    pub minimum: Option<u64>,
    pub currency: String,
    pub city: String,
}

/// A representation of `Talent` with limited fields.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FoundTalent {
    pub id: u32,
    pub headline: String,
    pub avatar_url: String,
    pub work_locations: Vec<String>,
    pub current_location: String,
    pub salary_expectations: Vec<SalaryExpectations>,
    pub roles_experiences: Vec<RolesExperience>,
    pub latest_position: String,
    pub batch_starts_at: String,
}

impl PartialEq<Talent> for FoundTalent {
    fn eq(&self, other: &Talent) -> bool {
        self.id == other.id
    }
}

impl PartialEq<FoundTalent> for Talent {
    fn eq(&self, other: &FoundTalent) -> bool {
        self.id == other.id
    }
}

impl<'a> PartialEq<u32> for &'a Talent {
    fn eq(&self, other: &u32) -> bool {
        self.id == *other
    }
}

/// A struct that joins `desired_work_roles` and `desired_work_roles_experience`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RolesExperience {
    pub role: String,
    pub experience: String,
}

impl RolesExperience {
    fn new<S: AsRef<str>>(role: &str, experience: Option<S>) -> RolesExperience {
        RolesExperience {
            role: role.to_owned(),
            experience: experience.map(|e| e.as_ref().into()).unwrap_or(String::new()),
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
            id: talent.id,
            headline: talent.headline.to_owned(),
            avatar_url: talent.avatar_url.to_owned(),
            work_locations: talent.work_locations.to_owned(),
            current_location: talent.current_location.to_owned(),
            salary_expectations: talent.salary_expectations.to_owned(),
            roles_experiences: roles_experiences,
            latest_position: talent.latest_position.to_owned(),
            batch_starts_at: talent.batch_starts_at.to_owned(),
        }
    }
}

/// The talent that will be indexed into ElasticSearch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Talent {
    pub id: u32,
    pub accepted: bool,
    #[serde(default)]
    pub desired_work_roles: Vec<String>,
    #[serde(default)]
    pub desired_work_roles_experience: Vec<String>, // experience in the desired work roles
    #[serde(default)]
    pub desired_roles: Vec<RolesExperience>,
    pub professional_experience: String,            // i.e. 2..6
    pub work_locations: Vec<String>,                // wants to work in
    pub current_location: String,                   // where the talent is based in
    pub work_authorization: String,                 // yes/no/unsure (visa)
    pub skills: Vec<String>,
    pub summary: String,
    pub headline: String,
    pub contacted_company_ids: Vec<u32>, // contacted companies
    pub batch_starts_at: String,
    pub batch_ends_at: String,
    pub added_to_batch_at: String,
    pub weight: i32,
    pub blocked_companies: Vec<u32>,
    pub work_experiences: Vec<String>, // past work experiences (i.e. ["Frontend developer", "SysAdmin"])
    pub avatar_url: String,
    pub salary_expectations: Vec<SalaryExpectations>,
    pub latest_position: String, // the very last experience_entries#position
    pub languages: Vec<String>,
    pub educations: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct DesiredRoleFilter<'a> {
    role: &'a str,
    minimum: Option<u8>,
    maximum: Option<u8>,
}

fn parse_desired_role_filter(input: &str) -> Option<DesiredRoleFilter> {
    let input = input.trim();
    if input.is_empty() {
        return None
    }

    let mut parts = input.split(":");

    parts.next().map(|role| {
        let minimum = parts.next().unwrap_or("").parse().ok();

        let maximum = minimum.and_then(|min| {
            parts.next()
                .unwrap_or("")
                .parse().ok()
                .filter(|&max| max >= min)
        });

        DesiredRoleFilter { role, minimum, maximum }
    })
}

fn mapped_experience_ranges(minimum: u8) -> Vec<&'static str> {
    static WORK_EXPERIENCE_MAPPING: &'static [&'static str] = &[
        "0..1",
        "0..1",
        "1..2",
        "2..4",
        "2..4",
        "4..6",
        "4..6",
        "6..8",
        "6..8",
        "8+"
    ];

    let min_idx = ::std::cmp::min(minimum, 9) as usize;
    let mut mappings = WORK_EXPERIENCE_MAPPING[min_idx..].to_vec();
    mappings.dedup();
    mappings
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
    pub fn visibility_filters(
        epoch: &str,
        presented_talents: Vec<i32>,
        date_filter_present: bool,
    ) -> Vec<Query> {
        let visibility_rules;

        if date_filter_present {
            visibility_rules = Query::build_bool()
                .with_must(vec![
                    Query::build_term("accepted", true).build(),
                    Query::build_term("batch_starts_at", epoch).build(),
                ])
                .build();
        } else {
            visibility_rules = Query::build_bool()
                .with_must(vec![
                    Query::build_term("accepted", true).build(),
                    Query::build_range("batch_starts_at")
                        .with_lte(epoch)
                        .with_format("dateOptionalTime")
                        .build(),
                    Query::build_range("batch_ends_at")
                        .with_gte(epoch)
                        .with_format("dateOptionalTime")
                        .build(),
                ])
                .build();
        }

        if !presented_talents.is_empty() {
            let presented_talents_filters = Query::build_bool()
                .with_must(
                    vec![<Query as VectorOfTerms<i32>>::build_terms(
                        "ids",
                        &presented_talents,
                    )].into_iter()
                        .flat_map(|x| x)
                        .collect::<Vec<Query>>(),
                )
                .build();
            vec![
                Query::build_bool()
                    .with_should(vec![visibility_rules, presented_talents_filters])
                    .build(),
            ]
        } else {
            vec![visibility_rules]
        }
    }

    pub fn salary_expectations_filters(params: &Map) -> Vec<Query> {
        if let Some(&Value::String(ref max_salary)) = params.get("maximum_salary") {
            let max_salary: u64 = match max_salary.parse().ok() {
                Some(max_salary) => max_salary,
                None => return vec![],
            };

            let mut salary_query =
                Query::build_nested(
                    "salary_expectations",
                    Query::build_range("salary_expectations.minimum")
                    .with_lte(max_salary)
                    .build()
                )
                .build();

            if !params.contains_key("work_locations") {
                return vec![salary_query];
            }
            let mut salary_location_query_terms = vec![];

            let work_locations: Vec<String> = vec_from_params!(params, "work_locations");
            for location in work_locations {
                salary_location_query_terms.push(
                    Query::build_nested(
                        "salary_expectations",
                        Query::build_bool()
                            .with_must(vec![
                                Query::build_range("salary_expectations.minimum")
                                    .with_lte(max_salary)
                                    .build(),
                                Query::build_term("salary_expectations.city", location)
                                .build()
                            ])
                            .build()
                    )
                    .build()
                )
            }

            salary_location_query_terms
        } else {
            vec![]
        }
    }

    pub fn desired_roles_filters(params: &Map) -> Vec<Query> {
        let mut terms = vec![];
        let mut basic_roles = vec![];

        let query_params: Vec<String> = vec_from_params!(params, "desired_work_roles");
        for filter in query_params.iter().map(AsRef::as_ref).filter_map(parse_desired_role_filter) {
            if let Some(minimum) = filter.minimum {
                terms.extend(
                    mapped_experience_ranges(minimum).into_iter().map(|mapped_range| {
                        Query::build_nested(
                            "desired_roles",
                            Query::build_bool()
                                .with_must(vec![
                                    Query::build_term("desired_roles.role", filter.role)
                                        .build(),
                                    Query::build_term("desired_roles.experience", mapped_range)
                                        .build()
                                ])
                                .build()
                        )
                        .build()
                    })
                );
            }  else {
                basic_roles.push(filter.role.into());
            }
        }

        if !basic_roles.is_empty() {
            terms.extend(
                <Query as VectorOfTerms<String>>::build_terms(
                    "desired_work_roles.raw",
                    &basic_roles
                )
            )
        }

        terms
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

        let search_features_param = params
            .get("features")
            .unwrap_or(&Value::Null);
        let search_features: Vec<String> = <_>::from_value(search_features_param).unwrap_or(vec![]);
        let search_features: HashSet<_> = search_features.into_iter().collect();
        println!("search_features: {:?}", search_features);

        let mut must_filters = vec![
            vec![
                Query::build_bool()
                    .with_must(
                        vec_from_params!(params, "languages")
                            .into_iter()
                            .map(|language: String| {
                                Query::build_term("languages", language).build()
                            })
                            .collect::<Vec<Query>>(),
                    )
                    .build(),
            ],
            <Query as VectorOfTerms<String>>::build_terms(
                "professional_experience",
                &vec_from_params!(params, "professional_experience"),
            ),
            <Query as VectorOfTerms<String>>::build_terms(
                "work_authorization",
                &vec_from_params!(params, "work_authorization"),
            ),
            <Query as VectorOfTerms<String>>::build_terms(
                "work_locations",
                &vec_from_params!(params, "work_locations"),
            ),
            <Query as VectorOfTerms<String>>::build_terms(
                "current_location",
                &vec_from_params!(params, "current_location"),
            ),
            <Query as VectorOfTerms<i32>>::build_terms(
                "id",
                &vec_from_maybe_csv_params!(params, "bookmarked_talents"),
            ),
            Talent::visibility_filters(
                epoch,
                i32_vec_from_params!(params, "presented_talents"),
                date_filter_present,
            ),
        ];

        let mut should_filters = vec![];
        let no_fulltext_search = search_features.contains("no_fulltext_search");

        let overrides = if no_fulltext_search {
            vec![
                ("summary", ".keyword"),
                ("headline", ".keyword"),
                ("skills", ".keyword"),
            ]
        } else {
            vec![]
        }.into_iter().collect();

        let keywords_use_should = search_features.contains("keywords_should");
        let keyword_filter = match Talent::full_text_search(params, overrides) {
            Some(keywords) => vec![keywords],
            None => vec![],
        };

        if keywords_use_should {
            should_filters.push(keyword_filter);
        } else {
            must_filters.push(keyword_filter);
        }

        Query::build_bool()
           .with_should(
                should_filters.into_iter()
                    .flat_map(|x| x)
                    .collect::<Vec<Query>>(),
            )
            .with_must(
                must_filters.into_iter()
                    .flat_map(|x| x)
                    .collect::<Vec<Query>>(),
            )
            .with_filter(
                Query::build_bool()
                    .with_must(
                        vec![
                            Query::build_bool()
                                .with_should(Talent::salary_expectations_filters(params))
                                .build(),
                            Query::build_bool()
                                .with_should(Talent::desired_roles_filters(params))
                                .build(),
                        ]
                    )
                    .build()
            )
            .with_must_not(
                vec![
                    <Query as VectorOfTerms<i32>>::build_terms(
                        "contacted_company_ids",
                        &company_id,
                    ),
                    <Query as VectorOfTerms<i32>>::build_terms("blocked_companies", &company_id),
                    <Query as VectorOfTerms<i32>>::build_terms(
                        "id",
                        &vec_from_maybe_csv_params!(params, "contacted_talents"),
                    ),
                    <Query as VectorOfTerms<i32>>::build_terms(
                        "id",
                        &vec_from_maybe_csv_params!(params, "ignored_talents"),
                    ),
                ].into_iter()
                    .flat_map(|x| x)
                    .collect::<Vec<Query>>(),
            )
            .build()
    }

    pub fn full_text_search(params: &Map, overrides: HashMap<&str, &str>) -> Option<Query> {
        match params.get("keywords") {
            Some(&Value::String(ref keywords)) => {
                if keywords.is_empty() {
                    return None;
                }

                // TODO: refactor me
                // This is a very bad approach but ATM I don't know
                // how to do exact matching on ngrams. My temptative
                // with build_bool().with_should() failed.
                let raw_query = keywords.contains('\"');
                macro_rules! maybe_raw {
                    ($field:expr) => {{
                        let field_modifier = overrides.get($field).unwrap_or(&"");
                        format!("{}{}{}", $field, field_modifier, if raw_query { ".raw" } else { "" })
                    }};
                }
                let query = Query::build_query_string(keywords.to_owned())
                    .with_fields(vec![
                        maybe_raw!("skills"),
                        maybe_raw!("summary"),
                        maybe_raw!("headline"),
                        maybe_raw!("desired_work_roles"),
                        maybe_raw!("work_experiences"),
                        maybe_raw!("educations"),
                    ])
                    .build();

                Some(query)
            }
            _ => None,
        }
    }

    /// Return a `Sort` that makes values be sorted for given fields, descendently.
    pub fn sorting_criteria() -> Sort {
        Sort::new(vec![
            SortField::new("batch_starts_at", Some(Order::Desc))
                .with_unmapped_type("date")
                .build(),
            SortField::new("weight", Some(Order::Desc))
                .with_unmapped_type("integer")
                .build(),
            SortField::new("added_to_batch_at", Some(Order::Desc))
                .with_unmapped_type("date")
                .build(),
        ])
    }
}

impl Resource for Talent {
    type Results = SearchResults;

    /// Populate the ElasticSearch index with `Vec<Talent>`
    fn index(es: &mut Client, index: &str, resources: Vec<Self>) -> Result<BulkResult, EsError> {
        fn sync_desired_work_roles(r: &mut Talent) {
            // Handle the future upgrade to only sending `desired_roles`
            if !r.desired_roles.is_empty() {
                r.desired_work_roles.clear();
                r.desired_work_roles_experience.clear();

                for role in r.desired_roles.iter() {
                    r.desired_work_roles.push(role.role.clone());
                    r.desired_work_roles_experience.push(role.experience.clone());
                }
            } else {
                let mut desired_roles = vec![];
                for (role, exp) in r.desired_work_roles.iter().zip(r.desired_work_roles_experience.iter()) {
                    desired_roles.push(RolesExperience::new(role, Some(exp)))
                }
                r.desired_roles = desired_roles;
            }
        }

        es.bulk(&resources
            .into_iter()
            .map(|mut r| {
                let id = r.id.to_string();
                sync_desired_work_roles(&mut r);
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
            _ => Utc::now().to_rfc3339(),
        };

        let index: Vec<&str> = match params.get("index") {
            Some(&Value::String(ref index)) => vec![&index[..]],
            _ => vec![default_index],
        };

        let keywords_present = match params.get("keywords") {
            Some(keywords) => match keywords {
                &Value::String(ref keywords) => !keywords.is_empty(),
                _ => false,
            },
            None => false,
        };

        let offset: u64 = match params.get("offset") {
            Some(&Value::String(ref offset)) => offset.parse().unwrap_or(0),
            Some(&Value::U64(ref offset)) => *offset,
            _ => 0,
        };

        let per_page: u64 = match params.get("per_page") {
            Some(&Value::String(ref per_page)) => per_page.parse().unwrap_or(10),
            Some(&Value::U64(ref per_page)) => *per_page,
            _ => 10,
        };

        let debug_es_query: bool = match params.get("debug_es_query") {
            Some(&Value::String(ref boolean)) => boolean == "true",
            _ => false,
        };

        let mut raw_es_query = None;
        let search_filters = &Talent::search_filters(params, &*epoch);

        let result = if keywords_present {
            let mut highlight = Highlight::new()
                .with_encoder(Encoders::HTML)
                .with_pre_tags(vec![String::new()])
                .with_post_tags(vec![String::new()])
                .to_owned();

            let settings = Setting::new()
                .with_type(SettingTypes::Plain)
                .with_term_vector(TermVector::WithPositionsOffsets)
                .with_fragment_size(1)
                .to_owned();

            match params.get("keywords") {
                Some(&Value::String(ref keywords)) => {
                    if keywords.contains("\"") {
                        highlight.add_setting("skills.raw".to_owned(), settings.clone());
                        highlight.add_setting("summary.raw".to_owned(), settings.clone());
                        highlight.add_setting("headline.raw".to_owned(), settings.clone());
                        highlight
                            .add_setting("desired_work_roles.raw".to_owned(), settings.clone());
                        highlight.add_setting("work_experiences.raw".to_owned(), settings.clone());
                        highlight.add_setting("educations.raw".to_owned(), settings.clone());
                    } else {
                        highlight.add_setting("skills".to_owned(), settings.clone());
                        highlight.add_setting("skills.keyword".to_owned(), settings.clone());
                        highlight.add_setting("summary".to_owned(), settings.clone());
                        highlight.add_setting("summary.keyword".to_owned(), settings.clone());
                        highlight.add_setting("headline".to_owned(), settings.clone());
                        highlight.add_setting("headline.keyword".to_owned(), settings.clone());
                        highlight.add_setting("desired_work_roles".to_owned(), settings.clone());
                        highlight.add_setting("work_experiences".to_owned(), settings.clone());
                        highlight.add_setting("educations".to_owned(), settings);
                    }
                }
                _ => {
                    highlight.add_setting("skills".to_owned(), settings.clone());
                    highlight.add_setting("skills.keyword".to_owned(), settings.clone());
                    highlight.add_setting("summary".to_owned(), settings.clone());
                    highlight.add_setting("summary.keyword".to_owned(), settings.clone());
                    highlight.add_setting("headline".to_owned(), settings.clone());
                    highlight.add_setting("headline.keyword".to_owned(), settings.clone());
                    highlight.add_setting("desired_work_roles".to_owned(), settings.clone());
                    highlight.add_setting("work_experiences".to_owned(), settings.clone());
                    highlight.add_setting("educations".to_owned(), settings);
                }
            }

            let mut query = es.search_query();

            let mut final_query = query.with_indexes(&*index)
                    .with_query(search_filters)
                    .with_highlight(&highlight)
                    .with_from(offset)
                    .with_size(per_page)
                    .with_min_score(0.56)
                    .with_track_scores(true);

            if debug_es_query {
                raw_es_query = final_query.es_query().ok();
            }
            final_query.send::<Talent>()
        } else {
            let sorting_criteria = &Talent::sorting_criteria();
            let mut query = es.search_query();

            let mut final_query = query.with_indexes(&*index)
                    .with_query(search_filters)
                    .with_sort(sorting_criteria)
                    .with_from(offset)
                    .with_size(per_page);

            if debug_es_query {
                raw_es_query = final_query.es_query().ok();
            }
            final_query.send::<Talent>()
        };

        match result {
            Ok(result) => {
                // println!("{:?}", result);
                let total = result.hits.total;

                if total == 0 {
                    return SearchResults {
                        raw_es_query: raw_es_query,
                        .. SearchResults::default()
                    }
                }

                let mut results: Vec<SearchResult> = result
                    .hits
                    .hits
                    .into_iter()
                    .map(SearchResult::from)
                    .collect();
                SearchResults {
                    total: total,
                    talents: results,
                    raw_es_query: raw_es_query,
                }
            }
            Err(err) => {
                error!("{:?}", err);
                SearchResults::default()
            }
        }
    }

    /// Delete the talent associated to given id.
    fn delete(es: &mut Client, id: &str, index: &str) -> Result<DeleteResult, EsError> {
        es.delete(index, ES_TYPE, id).send()
    }

    /// Reset the given index. All the data will be destroyed and then the index
    /// will be created again. The map that will be used is hardcoded.
    fn reset_index(mut es: &mut Client, index: &str) -> Result<MappingResult, EsError> {
        let mappings = json!({
      ES_TYPE: {
        "properties": {
          "id": {
            "type":  "integer",
            "index": "not_analyzed"
          },

          "desired_work_roles": {
            "type":            "string",
            "analyzer":        "trigrams",
            "search_analyzer": "words",
            "fields": {
              "raw": {
                "type": "string",
                "index": "not_analyzed"
              }
            }
          },

          "desired_work_roles_experience": {
            "type":  "string",
            "index": "not_analyzed"
          },

          "desired_roles": {
            "type":  "nested",
            "properties": {
                "role": { "type": "string", "index": "not_analyzed" },
                "experience": { "type": "string", "index": "not_analyzed" }
            }
          },

          "professional_experience": {
            "type":  "string",
            "index": "not_analyzed"
          },

          "work_locations": {
            "type":  "string",
            "index": "not_analyzed"
          },

          "educations": {
            "type":            "string",
            "analyzer":        "trigrams",
            "search_analyzer": "words",
            "fields": {
              "raw": {
                "type": "string"
              }
            }
          },

          "languages": {
            "type":  "string",
            "index": "not_analyzed"
          },

          "current_location": {
            "type":  "string",
            "index": "not_analyzed"
          },

          "work_authorization": {
            "type":  "string",
            "index": "not_analyzed"
          },

          "skills": {
            "type": "multi_field",
            "fields": {
                "skills": {
                    "type": "string",
                    "analyzer":        "trigrams",
                    "search_analyzer": "words",
                    "boost":           "2.0",
                },
                "keyword": {
                    "type": "string",
                    "analyzer":        "keywords",
                    "search_analyzer": "keywords",
                    "boost":           "2.0",
                },
                "raw": {
                    "type": "string",
                    "index": "not_analyzed"
                }
            }
          },

          "summary": {
            "type": "multi_field",
            "fields": {
                "summary": {
                    "type":            "string",
                    "analyzer":        "trigrams",
                    "search_analyzer": "words",
                    "boost":           "2.0",
                },
                "keyword": {
                    "type":            "string",
                    "analyzer":        "keywords",
                    "search_analyzer": "keywords",
                    "boost":           "2.0",
                },
                "raw": {
                    "type": "string",
                    "index": "not_analyzed"
                }
            }
          },

          "headline": {
            "type": "multi_field",
            "fields": {
                "headline": {
                    "type": "string",
                    "analyzer":        "trigrams",
                    "search_analyzer": "words",
                    "boost":           "2.0",
                },
                "keyword": {
                    "type": "string",
                    "analyzer":        "keywords",
                    "search_analyzer": "keywords",
                    "boost":           "2.0",
                },
                "raw": {
                    "type": "string",
                    "index": "not_analyzed"
                }
            }
          },

          "work_experiences": {
            "type":            "string",
            "analyzer":        "trigrams",
            "search_analyzer": "words",
            "fields": {
              "raw": {
                "type": "string",
                "index": "not_analyzed"
              }
            }
          },

          "contacted_company_ids": {
            "type":  "integer",
            "index": "not_analyzed"
          },

          "accepted": {
            "type":  "boolean",
            "index": "not_analyzed"
          },

          "batch_starts_at": {
            "type":   "date",
            "format": "dateOptionalTime",
            "index":  "not_analyzed"
          },

          "batch_ends_at": {
            "type":   "date",
            "format": "dateOptionalTime",
            "index":  "not_analyzed"
          },

          "added_to_batch_at": {
            "type":   "date",
            "format": "dateOptionalTime",
            "index":  "not_analyzed"
          },

          "weight": {
            "type":  "integer",
            "index": "not_analyzed"
          },

          "blocked_companies": {
            "type":  "integer",
            "index": "not_analyzed"
          },

          "avatar_url": {
            "type":  "string",
            "index": "not_analyzed"
          },

          "salary_expectations": {
            "type":  "nested",
            "properties": {
                "minimum": { "type": "long", "index": "not_analyzed" },
                "city": { "type": "string", "index": "not_analyzed" },
                "currency": { "type": "string", "index": "not_analyzed" }
            }
          },

          "latest_position": {
            "type":  "string",
            "index": "not_analyzed"
          }
        }
      }
    });

        let settings = Settings {
            number_of_shards: 1,

            analysis: Analysis {
                filter: json!({
          "trigrams_filter": {
            "type":     "ngram",
            "min_gram": 2,
            "max_gram": 20
          },

          "words_splitter": {
            "type":              "word_delimiter",
            "preserve_original": true,
            "catenate_all":      true
          },

          "english_words_filter": {
            "type":      "stop",
            "stopwords": "_english_"
          },

          "tech_words_filter": {
            "type":      "stop",
            "stopwords": ["js"]
          },

          "strip_js": {
              "type": "pattern_replace",
              // Lazy match on the initial match so the '.' can be captured by the optional \\.?
              "pattern": "(.*?)\\.?js\\z",
              "replacement": "$1",
          },

          "protect_keywords": {
              "type": "keyword_marker",
              "keywords": [
                  "C++", "C#"
              ],
              "ignore_case": true,
          },
        }).as_object()
                    .unwrap()
                    .to_owned(),
                analyzer: json!({
          "trigrams": { // index time
            "type":      "custom",
            "tokenizer": "whitespace",
            "filter":    ["lowercase", "words_splitter", "trigrams_filter",
                           "english_words_filter", "tech_words_filter"]
          },
          "words": { // query time
            "type":      "custom",
            "tokenizer": "keyword",
            "filter":    ["lowercase", "words_splitter", "english_words_filter",
                           "tech_words_filter"]
          },
          "keywords": {
            "type":      "custom",
            "tokenizer": "standard",
            "filter":    ["lowercase", "protect_keywords", "trim", "english_words_filter",
                            "strip_js"]
          }
        }).as_object()
                    .unwrap()
                    .to_owned(),
            },
        };

        if let Err(error) = es.delete_index(index) {
            error!("{}", error);
        }

        MappingOperation::new(&mut es, index)
            .with_mappings(&mappings)
            .with_settings(&settings)
            .send()
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_desired_role_filter, mapped_experience_ranges, DesiredRoleFilter, RolesExperience};
    use serde_json;
    use resources::Talent;

    #[test]
    fn parsing_desired_roles() {
        fn check<'a>(input: u8, expected: &[&str]) {
            assert_eq!(mapped_experience_ranges(input), expected)
        }

        vec![
            (30, vec!["8+"]),
            (9, vec!["8+"]),
            (8, vec!["6..8", "8+"]),
            (7, vec!["6..8", "8+"]),
            (6, vec!["4..6", "6..8", "8+"]),
            (5, vec!["4..6", "6..8", "8+"]),
            (4, vec!["2..4", "4..6", "6..8", "8+"]),
            (3, vec!["2..4", "4..6", "6..8", "8+"]),
            (2, vec!["1..2", "2..4", "4..6", "6..8", "8+"]),
            (1, vec!["0..1", "1..2", "2..4", "4..6", "6..8", "8+"]),
            (0, vec!["0..1", "1..2", "2..4", "4..6", "6..8", "8+"]),
        ].into_iter()
        .for_each(|(input, expected)| check(input, &expected))
    }

    #[test]
    fn experience_range_mapping() {
        fn check<'a>(input: &'a str, expected: DesiredRoleFilter<'a>) {
            assert_eq!(parse_desired_role_filter(input), Some(expected))
        }

        vec![
            ("foobar", ("foobar", None, None)),
            ("ruby:5", ("ruby", Some(5), None)),
            ("ruby:five", ("ruby", None, None)),
            ("ruby:5:10", ("ruby", Some(5), Some(10))),
            ("ruby:5:ten", ("ruby", Some(5), None)),
            ("ruby:five:10", ("ruby", None, None)),
            ("ruby:5:2", ("ruby", Some(5), None)),
            ("ruby:5:5", ("ruby", Some(5), Some(5))),
        ].into_iter()
        .map(|(s, (role, minimum, maximum))| (s, DesiredRoleFilter { role, minimum, maximum }))
        .for_each(|(input, expected)| check(input, expected))
    }

    #[test]
    fn parsing_empty_desired_roles() {
        assert_eq!(parse_desired_role_filter(""), None);
        assert_eq!(parse_desired_role_filter("   "), None);
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
      \"educations\":[\"CS\"],
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
      \"salary_expectations\": [
          {\"minimum\": 40000, \"maximum\": 50000, \"currency\": \"EUR\", \"city\": \"Berlin\"},
          {\"minimum\": 20000, \"maximum\": null, \"currency\": \"EUR\", \"city\": \"Amsterdam\"},
          [30000, \"EUR\", \"Frankfurt\"]
       ],
      \"latest_position\":\"Developer\",
      \"languages\":[\"English\"]
    }".to_owned();

        let resource: Result<Talent, _> = serde_json::from_str(&payload);
        let resource = resource.unwrap();
        assert_eq!(resource.desired_work_roles, vec!["C/C++ Engineer"]);
    }

    #[test]
    fn test_json_decode_with_structured_desired_work_roles() {
        let payload = r##"{
      "id":13,
      "desired_work_roles":["C/C++ Engineer"],
      "desired_work_roles_experience":["2..4"],
      "desired_roles": [
          { "role": "C/C++ Engineer", "experience": "2..4" },
          { "role": "DevOps", "experience": "8+" }
      ],
      "work_languages":["C++"],
      "professional_experience":"8+",
      "work_locations":["Berlin"],
      "educations":["CS"],
      "current_location":"Berlin",
      "work_authorization":"yes",
      "skills":["Rust"],
      "summary":"Blabla",
      "headline":"I see things, I do stuff",
      "contacted_company_ids":[1],
      "accepted":true,
      "batch_starts_at":"2016-03-04T12:24:00+01:00",
      "batch_ends_at":"2016-04-11T12:24:00+02:00",
      "added_to_batch_at":"2016-03-11T12:24:37+01:00",
      "weight":0,
      "blocked_companies":[99],
      "work_experiences":["Frontend developer", "SysAdmin"],
      "avatar_url":"https://secure.gravatar.com/avatar/47ac43379aa70038a9adc8ec88a1241d?s=250&d=https%3A%2F%2Fsecure.gravatar.com%2Favatar%2Fa0b9ad63fb35d210a218c317e0a6284e%3Fs%3D250",
      "salary_expectations": [
          {"minimum": 40000, "maximum": 50000, "currency": "EUR", "city": "Berlin"},
          {"minimum": 20000, "maximum": null, "currency": "EUR", "city": "Amsterdam"},
          [30000, "EUR", "Frankfurt"]
       ],
      "latest_position":"Developer",
      "languages":["English"]
    }"##.to_owned();

        let resource: Result<Talent, _> = serde_json::from_str(&payload);
        let resource = resource.unwrap();
        assert_eq!(
            resource.desired_roles,
            vec![
                RolesExperience { role: "C/C++ Engineer".into(), experience: "2..4".into() },
                RolesExperience { role: "DevOps".into(), experience: "8+".into() }
            ]
        );
    }

    #[test]
    fn test_json_decode_without_old_desired_work_roles() {
        let payload = r##"{
      "id":13,
      "desired_roles": [
          { "role": "C/C++ Engineer", "experience": "2..4" },
          { "role": "DevOps", "experience": "8+" }
      ],
      "work_languages":["C++"],
      "professional_experience":"8+",
      "work_locations":["Berlin"],
      "educations":["CS"],
      "current_location":"Berlin",
      "work_authorization":"yes",
      "skills":["Rust"],
      "summary":"Blabla",
      "headline":"I see things, I do stuff",
      "contacted_company_ids":[1],
      "accepted":true,
      "batch_starts_at":"2016-03-04T12:24:00+01:00",
      "batch_ends_at":"2016-04-11T12:24:00+02:00",
      "added_to_batch_at":"2016-03-11T12:24:37+01:00",
      "weight":0,
      "blocked_companies":[99],
      "work_experiences":["Frontend developer", "SysAdmin"],
      "avatar_url":"https://secure.gravatar.com/avatar/47ac43379aa70038a9adc8ec88a1241d?s=250&d=https%3A%2F%2Fsecure.gravatar.com%2Favatar%2Fa0b9ad63fb35d210a218c317e0a6284e%3Fs%3D250",
      "salary_expectations": [
          {"minimum": 40000, "maximum": 50000, "currency": "EUR", "city": "Berlin"},
          {"minimum": 20000, "maximum": null, "currency": "EUR", "city": "Amsterdam"},
          [30000, "EUR", "Frankfurt"]
       ],
      "latest_position":"Developer",
      "languages":["English"]
    }"##.to_owned();

        let resource: Result<Talent, _> = serde_json::from_str(&payload);
        let resource = resource.unwrap();
        assert_eq!(
            resource.desired_roles,
            vec![
                RolesExperience { role: "C/C++ Engineer".into(), experience: "2..4".into() },
                RolesExperience { role: "DevOps".into(), experience: "8+".into() }
            ]
        );
    }
}
