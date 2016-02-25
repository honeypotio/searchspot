extern crate rustc_serialize;

use std::fs::File;
use std::io::prelude::*;
use toml::{Parser, Value};
use toml;

#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct DBConfig {
  pub uri: String
}

impl DBConfig {
  pub fn new() -> DBConfig {
    DBConfig {
      uri: "postgres://lando@localhost/lando_development".to_owned(),
    }
  }
}

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
      port: 9000,
      indexes: vec!["honeypot_dev_talents".to_owned()]
    }
  }
}

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

#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct Config {
  pub db:   DBConfig,
  pub es:   ESConfig,
  pub http: HTTPConfig
}

impl Config {
  /// Return a new `Config` fill with the default values
  pub fn new() -> Config {
    Config {
      db:   DBConfig::new(),
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
    [db]
    uri = "postgres://homu@madoka/incubator"

    [es]
    host    = "123.0.123.0"
    port    = 9000
    indexes = ["honeypot_dev_talents"]

    [http]
    host = "1.0.0.127"
    port = 3000
  "#;

  #[test]
  fn test_config() {
    let config = Config::new();
    assert_eq!(config.db.uri,
                 "postgres://lando@localhost/lando_development".to_owned());
    assert_eq!(config.es.host,   "localhost".to_owned());
    assert_eq!(config.http.host, "127.0.0.1".to_owned());
  }

  #[test]
  fn test_parse() {
    let config = Config::parse(Some(sample_config.to_owned()));
    assert_eq!(config.db.uri,    "postgres://homu@madoka/incubator".to_owned());
    assert_eq!(config.es.host,   "123.0.123.0".to_owned());
    assert_eq!(config.http.host, "1.0.0.127".to_owned());
  }
}
