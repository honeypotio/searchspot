use toml::{self, Parser, Value};

use std::fs::File;
use std::io::prelude::*;
use std::fmt;

/// Contain the configuration for ElasticSearch.
#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct ESConfig {
  pub host: String,
  pub port: u32,
  pub indexes: Vec<String>
}

impl ESConfig {
  pub fn new() -> ESConfig {
    ESConfig {
      host: "localhost".to_owned(),
      port: 9200,
      indexes: vec!["my_index".to_owned()]
    }
  }
}

impl fmt::Display for ESConfig {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "ElasticSearch on {}:{} (indexes: {})",
      self.host, self.port, self.indexes.join(", "))
  }
}

/// Contain instructions about where Honeysearch must
/// listen to for new connections.
#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct HTTPConfig {
  pub host: String,
  pub port: u32
}

impl HTTPConfig {
  pub fn new() -> HTTPConfig {
    HTTPConfig {
      host: "127.0.0.1".to_owned(),
      port: 3000,
    }
  }
}

impl fmt::Display for HTTPConfig {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Listening on http://{}...", self.host)
  }
}

/// Container for ESConfig and HTTPConfig.
#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct Config {
  pub http: HTTPConfig,
  pub es:   ESConfig
}

impl Config {
  /// Return a new `Config` fill with the default values
  pub fn new() -> Config {
    Config {
      es:   ESConfig::new(),
      http: HTTPConfig::new()
    }
  }

  /// Load, parse and return the configuration file
  /// wrapped inside a `Config`.
  pub fn load_config(path: String) -> Config {
    let config_toml = Config::read_file(path).clone();
    Config::parse(config_toml)
  }

  /// Read a file from the given path and return its content
  pub fn read_file(path: String) -> Option<String> {
    let mut config_toml = String::new();

    let mut file = match File::open(&path) {
      Ok(file) => file,
      Err(_)   => return None
    };

    file.read_to_string(&mut config_toml)
        .unwrap_or_else(|err| panic!("Error while reading config: [{}]", err));

    Some(config_toml)
  }

  /// Parse given TOML configuration file and return it
  /// wrapped inside a `Config`.
  pub fn parse(config_toml: Option<String>) -> Config {
    if config_toml.is_none() {
      println!("{} {}", "Requested configuration file cannot be found.",
                        "The default configuration will be loaded.\n");
      return Config::new();
    }

    let config_toml_ = config_toml.unwrap();
    let mut parser   = Parser::new(&*config_toml_);
    let     toml     = parser.parse();

    match toml {
      Some(config) => {
        let config = Value::Table(config);
        toml::decode(config).unwrap()
      },
      None => {
        println!("{:?}", parser.errors);
        panic!("Error while parsing the configuration file.");
      }
    }
  }
}

#[cfg(test)]
#[allow(non_upper_case_globals)]
mod tests {
  use config::*;

  const sample_config: &'static str = r#"
    [es]
    host    = "123.0.123.0"
    port    = 9000
    indexes = ["save_meguka"]

    [http]
    host = "1.0.0.127"
    port = 3000
  "#;

  #[test]
  fn test_new() {
    // returns a Config fill with the default hardcoded data
    let config = Config::new();
    assert_eq!(config.es.host,   "localhost".to_owned());
    assert_eq!(config.http.host, "127.0.0.1".to_owned());
  }

  #[test]
  fn test_parse() {
    // returns a Config fill with given TOML configuration file
    let config = Config::parse(Some(sample_config.to_owned()));
    assert_eq!(config.es.host,   "123.0.123.0".to_owned());
    assert_eq!(config.http.host, "1.0.0.127".to_owned());
  }
}
