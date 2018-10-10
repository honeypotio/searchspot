extern crate searchspot;
extern crate rs_es;
extern crate chrono;
extern crate params;
extern crate serde_json;
#[macro_use]
extern crate lazy_static;
extern crate urlencoded;
extern crate url;

use searchspot::resources::Talent;
use searchspot::resources::SearchResults;
use searchspot::resource::Resource;
use chrono::prelude::*;

use rs_es::operations::search::highlight::HighlightResult;
use rs_es::Client;

use params::Value;

use helpers::{make_client, refresh_index, CONFIG, parse_query};

mod helpers {
    use urlencoded;
    use rs_es::Client;
    use params::{Map, Value};
    use url::form_urlencoded;

    use searchspot::config::Config;

    use std::collections::HashMap;

    const CONFIG_FILE: &'static str = "examples/tests.toml";

    lazy_static! {
        pub static ref CONFIG: Config = Config::from_file(CONFIG_FILE.to_owned());
    }

    pub fn make_client() -> Client {
        println!("Connecting client: {:?}", CONFIG.es.url);
        Client::new(&*CONFIG.es.url).unwrap()
    }

    pub fn refresh_index(client: &mut Client, index: &str) {
        client.refresh().with_indexes(&[&index]).send().unwrap();
    }

    // FIXME: this is relying a lot on implementation but I need a better api in order to fix
    // Based on: https://github.com/iron/params/blob/ba3ebf8390bc60d8d54f05d7de45d3abe93f3459/src/lib.rs#L613-L623
    pub fn parse_query<S: AsRef<str>>(query: S) -> Map {
        let raw = query.as_ref();
        let encoded = form_urlencoded::byte_serialize(raw.as_bytes())
            .collect::<Vec<&str>>()
            .concat()
            // reverse the double encode of actual param seperators
            .replace("%3D", "=").replace("%26", "&");

        parse_query_url_encoded(&encoded)
    }

    pub fn parse_query_url_encoded(query: &str) -> Map {
        let mut map = Map::new();

        println!("query: {:?}", query);
        let hash_map = match urlencoded::parse(query) {
            Ok(hash_map) => hash_map,
            Err(urlencoded::UrlDecodingError::EmptyQuery) => HashMap::new(),
            err => err.expect(&format!("Failed to parse query: {:?}", query)),
        };

        for (path, vec) in hash_map {
            for value in vec {
                map.assign(&path, Value::String(value))
                    .expect(&format!("Failed to assign to {:?}", path));
            }
        }

        map
    }
}

macro_rules! epoch_from_year {
    ($year:expr) => {
        Utc.datetime_from_str(&format!("{}-01-01 12:00:00", $year), "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .to_rfc3339()
    };
}

trait SearchResultsExt {
    fn ids(&self) -> Vec<u32>;
    fn highlights(&self) -> Vec<Option<HighlightResult>>;
    fn is_empty(&self) -> bool;
}

impl SearchResultsExt for SearchResults {
    fn ids(&self) -> Vec<u32> {
        self.talents.iter().map(|r| r.talent.id).collect()
    }

    fn highlights(&self) -> Vec<Option<HighlightResult>> {
        self.talents.iter().map(|r| r.highlight.clone()).collect()
    }

    fn is_empty(&self) -> bool {
        self.talents.is_empty()
    }
}

pub fn populate_index(mut client: &mut Client, index: &str) {
    let talents: Vec<Talent> = serde_json::from_str(r#"[
        {
            "id": 1,
            "accepted": true,
            "desired_work_roles": [],
            "desired_work_roles_experience": [],
            "desired_roles": [],
            "professional_experience": "1..2",
            "work_locations": ["Berlin"],
            "educations": ["Computer science"],
            "current_location": "Berlin",
            "work_authorization": "yes",
            "skills": ["Rust", "HTML5", "HTML"],
            "summary": "I'm a senior Rust developer and sometimes I do also HTML.",
            "headline": "Backend developer with Rust experience",
            "work_experiences": ["Database Administrator"],
            "contacted_company_ids": [],
            "batch_starts_at": "2006-01-01T12:00:00+00:00",
            "batch_ends_at": "2020-01-01T12:00:00+00:00",
            "added_to_batch_at": "2006-01-01T12:00:00+00:00",
            "weight": -5,
            "blocked_companies": [],
            "avatar_url": "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250",
            "salary_expectations": [
                [40000, "EUR", "Berlin"]
            ],
            "latest_position": "Developer",
            "languages": ["Italian"]
        },
        {
            "id": 2,
            "accepted": true,
            "desired_work_roles": [],
            "desired_work_roles_experience": [],
            "desired_roles": [],
            "professional_experience": "8+",
            "work_locations": ["Rome", "Berlin"],
            "educations": ["Computer science"],
            "current_location": "Berlin",
            "work_authorization": "yes",
            "skills": [
                "Rust",
                "HTML5",
                "Java",
                "Unity"
            ],
            "summary": "I'm a java dev with some tricks up my sleeves",
            "headline": "Senior Java engineer",
            "work_experiences": [],
            "contacted_company_ids": [],
            "batch_starts_at": "2006-01-01T12:00:00+00:00",
            "batch_ends_at": "2020-01-01T12:00:00+00:00",
            "added_to_batch_at": "2006-01-01T12:00:00+00:00",
            "weight": 6,
            "blocked_companies": [22],
            "avatar_url": "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250",
            "salary_expectations": [
                [30000, "EUR", "Berlin"]
            ],
            "latest_position": "",
            "languages": ["German", "English"]
        },
        {
            "id": 3,
            "accepted": false,
            "desired_work_roles": [],
            "desired_work_roles_experience": [],
            "desired_roles": [],
            "professional_experience": "1..2",
            "work_locations": ["Berlin"],
            "educations": ["Computer science"],
            "current_location": "Berlin",
            "work_authorization": "yes",
            "skills": [],
            "summary": "",
            "headline": "",
            "work_experiences": [],
            "contacted_company_ids": [],
            "batch_starts_at": "2007-01-01T12:00:00+00:00",
            "batch_ends_at": "2020-01-01T12:00:00+00:00",
            "added_to_batch_at": "2011-01-01T12:00:00+00:00",
            "weight": 6,
            "blocked_companies": [],
            "avatar_url": "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250",
            "salary_expectations": [
                [25000, "EUR", "Berlin"]
            ],
            "latest_position": "",
            "languages": ["English"]
        },
        {
            "id": 4,
            "accepted": true,
            "desired_work_roles": [],
            "desired_work_roles_experience": [],
            "desired_roles": [
                { "role": "Fullstack", "experience": "2..4" },
                { "role": "DevOps", "experience": "4..6" }
            ],
            "professional_experience": "1..2",
            "work_locations": ["Berlin"],
            "educations": ["Computer science", "Europe community"],
            "current_location": "Berlin",
            "work_authorization": "no",
            "skills": [
                "ClojureScript",
                "C++",
                "React.js"
            ],
            "summary": "ClojureScript right now, previously C++",
            "headline": "Senior fullstack developer with sysadmin skills.",
            "work_experiences": [
                "Backend Engineer",
                "Database Administrator"
            ],
            "contacted_company_ids": [6],
            "batch_starts_at": "2008-01-01T12:00:00+00:00",
            "batch_ends_at": "2020-01-01T12:00:00+00:00",
            "added_to_batch_at": "2011-01-01T12:00:00+00:00",
            "weight": 0,
            "blocked_companies": [],
            "avatar_url": "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250",
            "salary_expectations": [
                [40000, "EUR", "Berlin"]
            ],
            "latest_position": "",
            "languages": ["English"]
        },
        {
            "id": 5,
            "accepted": true,
            "desired_work_roles": ["Fullstack", "DevOps"],
            "desired_work_roles_experience": ["0..1", "8+"],
            "desired_roles": [
                { "role": "Fullstack", "experience": "0..1" },
                { "role": "DevOps", "experience": "8+" }
            ],
            "professional_experience": "1..2",
            "work_locations": ["Berlin", "Amsterdam"],
            "educations": [],
            "current_location": "Naples",
            "work_authorization": "yes",
            "skills": [
                "JavaScript",
                "C++",
                "Ember.js"
            ],
            "summary": "C++ and frontend dev. HTML, C++, JavaScript and C#. Did I say C++?",
            "headline": "Amazing C and Unity3D developer",
            "work_experiences": [],
            "contacted_company_ids": [6],
            "batch_starts_at": "2008-01-01T12:00:00+00:00",
            "batch_ends_at": "2020-01-01T12:00:00+00:00",
            "added_to_batch_at": "2011-01-01T12:00:00+00:00",
            "weight": 0,
            "blocked_companies": [],
            "avatar_url": "https://secure.gravatar.com/avatar/a0b9ad63fb35d210a218c317e0a6284e.jpg?s=250",
            "salary_expectations": [
                [10000, "EUR", "Amsterdam"]
            ],
            "latest_position": "",
            "languages": ["English"]
        }
    ]"#).unwrap();

    Talent::index(&mut client, &index, talents).unwrap();
}

#[test]
fn test_search() {
    let mut client = make_client();
    let index = format!("{}_{}", CONFIG.es.index, "talent");

    Talent::reset_index(&mut client, &*index).unwrap();

    refresh_index(&mut client, &*index);

    println!("populating: {}", index);
    populate_index(&mut client, &*index);
    refresh_index(&mut client, &*index);

    let empty_params = &parse_query("");

    // no parameters are given
    {
        let results = Talent::search(&mut client, &*index, empty_params);
        assert_eq!(vec![4, 5, 2, 1], results.ids());
        assert_eq!(4, results.total);
        assert!(results.highlights().iter().all(|r| r.is_none()));
    }

    {
        assert!(Talent::delete(&mut client, "1", &*index).is_ok());
        assert!(Talent::delete(&mut client, "4", &*index).is_ok());
        refresh_index(&mut client, &*index);

        let results = Talent::search(&mut client, &*index, empty_params);
        assert_eq!(vec![5, 2], results.ids());

        populate_index(&mut client, &*index);
        refresh_index(&mut client, &*index);
    }

    // a non existing index is given
    {
        let params = parse_query("index=lololol");
        let results = Talent::search(&mut client, &*index, &params);
        assert!(results.is_empty());
    }

    // a date that doesn't match given indexes is given
    {
        let params = parse_query(format!("epoch={}", epoch_from_year!("2040")));
        let results = Talent::search(&mut client, &*index, &params);
        assert!(results.is_empty());
    }

    // a date that match only some talents is given
    {
        let params = parse_query(format!("epoch={}", epoch_from_year!("2006")));
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2, 1], results.ids());
    }

    // page is given
    {
        let mut params = parse_query("per_page=2&offset=0");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5], results.ids());

        params.assign("offset", Value::U64(2)).unwrap();
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2, 1], results.ids());

        params.assign("offset", Value::U64(4)).unwrap();
        let results = Talent::search(&mut client, &*index, &params);
        assert!(results.ids().is_empty());
    }

    // searching for work roles
    {
        let params = parse_query("desired_work_roles[]=Fullstack");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5], results.ids());
    }

    // searching for work roles with experience ranges
    {
        let params = parse_query("desired_work_roles[]=Fullstack:2");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4], results.ids());

        // Works as an OR filter
        let params = parse_query("desired_work_roles[]=Fullstack:2&desired_work_roles[]=DevOps:0");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5], results.ids());

        // Ensure it still works with salary range filter
        let params = parse_query("desired_work_roles[]=Fullstack:2&desired_work_roles[]=DevOps:0\
                                    &maximum_salary=30000&work_locations[]=Amsterdam");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![5], results.ids());

        assert_eq!(results.raw_es_query, None);

        let params = parse_query("debug_es_query=true\
            &desired_work_roles[]=Fullstack:2\
            &desired_work_roles[]=DevOps:0\
            &maximum_salary=30000\
            &work_locations[]=Amsterdam");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![5], results.ids());
        assert!(
            results.raw_es_query.as_ref().unwrap()
                .contains("POST /sample_index_talent/_search"),
            "actual: {:?}",
            results.raw_es_query
        );
    }

    // searching for work experience
    {
        let params = parse_query("professional_experience[]=8+");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2], results.ids());
    }

    // searching for work locations
    {
        let params = parse_query("work_locations[]=Rome");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2], results.ids());
    }

    // searching for a language
    {
        let params = parse_query("languages[]=English");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5, 2], results.ids());
    }

    // searching for languages
    {
        let params = parse_query("languages[]=English\
            &languages[]=German");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2], results.ids());
    }

    // searching for a single keyword
    {
        let params = parse_query("keywords=HTML5");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![1, 2, 5], results.ids());
    }

    // searching for a keyword for education entries
    {
        let params = parse_query("keywords=computer science");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![1, 2, 4], results.ids());
    }

    // searching for a single, differently cased and incomplete keyword
    {
        let params = parse_query("keywords=html");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![1, 2, 5], results.ids());
    }

    // searching for keywords and filters
    {
        let params = parse_query("keywords=Rust, HTML5 and HTML\
            &work_locations[]=Rome");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2], results.ids());
    }

    // conditional search
    {
        let params = parse_query("keywords=C++ and Ember.js AND NOT React.js");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![5], results.ids());
    }

    {
        let params = parse_query("keywords=\"Unity\"");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2], results.ids());
    }

    // searching for a single word that's supposed to be split
    {
        let params = parse_query("keywords=reactjs");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4], results.ids());
    }

    // searching for the original dotted string
    {
        let params = parse_query("keywords=react.js\
            &work_locations[]=Berlin\
            &desired_work_roles[]=Fullstack");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4], results.ids());
    }

    // searching for a non-matching keyword
    {
        let params = parse_query("keywords=Criogenesi");
        let results = Talent::search(&mut client, &*index, &params);
        assert!(results.is_empty());
    }

    // searching for an empty keyword
    {
        let params = parse_query("keywords=");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5, 2, 1], results.ids());
    }

    // searching for different parts of a single keyword
    // (Java, JavaScript)
    {
        // JavaScript, Java
        {
            let params =parse_query("keywords=Java");
            let results = Talent::search(&mut client, &*index, &params);
            assert_eq!(vec![2, 5], results.ids());
        }

        // JavaScript
        {
            let params = parse_query("keywords=javascript");
            let results = Talent::search(&mut client, &*index, &params);
            assert_eq!(vec![5], results.ids());
        }

        // JavaScript, ClojureScript
        {
            let params = parse_query("keywords=script");
            let results = Talent::search(&mut client, &*index, &params);
            assert_eq!(vec![4, 5], results.ids());
        }
    }

    // Searching for summary
    {
        {
            let params = parse_query("keywords=right now");
            let results = Talent::search(&mut client, &*index, &params);
            assert_eq!(vec![4], results.ids());
        }

        {
            let params = parse_query("keywords=C++");
            let results = Talent::search(&mut client, &*index, &params);
            assert_eq!(vec![4, 5], results.ids());
        }

        {
            let params = parse_query("keywords=C#");
            let results = Talent::search(&mut client, &*index, &params);
            assert_eq!(vec![5], results.ids());
        }

        {
            let params = parse_query("keywords=rust and");
            let results = Talent::search(&mut client, &*index, &params);
            assert_eq!(vec![1, 2], results.ids());
        }
    }

    // Searching for headline and summary
    {
        let params = parse_query("keywords=senior");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2, 1, 4], results.ids());
    }

    // Searching for ideal work roles
    {
        let params = parse_query("keywords=Devops");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5], results.ids());
    }

    // Searching for previous job title
    {
        let params = parse_query("keywords=database admin");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 1], results.ids());
    }

    // Ignoring some talents
    {
        let params = parse_query("keywords=database admin\
            &ignored_talents[]=1");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4], results.ids());
    }

    // Ignoring some talents, csv parsing
    {
        let params = parse_query("keywords=database admin\
            &ignored_talents[]=1");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4], results.ids());

        let params = parse_query("keywords=database admin\
            &ignored_talents=1, 4");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(Vec::<u32>::new(), results.ids());
    }

    // highlight
    {
        let params = parse_query("keywords=C#");
        let results = Talent::search(&mut client, &*index, &params).talents;
        let highlights = results
            .into_iter()
            .map(|r| r.highlight.unwrap())
            .collect::<Vec<HighlightResult>>();
        assert_eq!(Some(&vec![" C#.".to_owned()]), highlights[0].get("summary"));
    }

    // filtering for given company_id (skip contacted talents)
    {
        let params = parse_query("company_id=6");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2, 1], results.ids());
    }

    // filtering for given bookmarks (ids)
    {
        let params = parse_query("bookmarked_talents[]=2\
            &bookmarked_talents[]=4\
            &bookmarked_talents[]=1\
            &bookmarked_talents[]=3\
            &bookmarked_talents[]=5\
            &bookmarked_talents[]=6\
            &bookmarked_talents[]=7\
            &bookmarked_talents[]=8");

        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5, 2, 1], results.ids());
        assert_eq!(4, results.total);

        let params = parse_query("bookmarked_talents[]=2\
            &bookmarked_talents[]=4");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 2], results.ids());
        assert_eq!(2, results.total);
    }

    // filtering for given bookmarks (ids) with csv parsing
    {
        let params = parse_query("bookmarked_talents=2,4,1,3,5,6,7,8");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5, 2, 1], results.ids());
        assert_eq!(4, results.total);

        let params = parse_query("bookmarked_talents=2,4");

        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 2], results.ids());
        assert_eq!(2, results.total);
    }

    // filtering for current_location
    {
        let params = parse_query("current_location[]=Naples");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![5], results.ids());
    }

    // filtering for work_authorization
    {
        let params = parse_query("work_authorization[]=no");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4], results.ids());
    }

    // ignoring contacted talents
    {
        let params = parse_query("contacted_talents[]=2");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5, 1], results.ids());
    }

    // ignoring contacted talents - csv parsing
    {
        let params = parse_query("contacted_talents=2,4");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![5, 1], results.ids());

        let params = parse_query("contacted_talents=2,5,4");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![1], results.ids());
    }

    // ignoring blocked companies
    {
        let params = parse_query("company_id=22");
        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![4, 5, 1], results.ids());
    }

    // search by maximum salary
    {
        let params = parse_query("maximum_salary=30000");
        let results = Talent::search(&mut client, &*index, &params);
        // ignores talent 3 due to accepted == false
        assert_eq!(vec![5, 2], results.ids());
    }

    // maximum salary searches should be scoped by location
    {
        let params = parse_query("maximum_salary=30000\
            &work_locations[]=Berlin");

        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![2], results.ids());

        let params = parse_query("maximum_salary=30000\
            &work_locations[]=Amsterdam");

        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![5], results.ids());

        // Ensure that work_locations are additive
        let params = parse_query("maximum_salary=30000\
            &work_locations[]=Amsterdam\
            &work_locations[]=Berlin");

        let results = Talent::search(&mut client, &*index, &params);
        assert_eq!(vec![5, 2], results.ids());
    }
}