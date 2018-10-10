mod talent;
pub use self::talent::Talent;

mod score;
pub use self::score::Score;

#[cfg(test)]
mod tests {
    use urlencoded;
    use rs_es::Client;
    use params::{Map, Value};
    use url::form_urlencoded;

    use config::Config;

    use std::collections::HashMap;

    const CONFIG_FILE: &'static str = "examples/tests.toml";

    lazy_static! {
        pub static ref CONFIG: Config = Config::from_file(CONFIG_FILE.to_owned());
    }

    pub fn make_client() -> Client {
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
