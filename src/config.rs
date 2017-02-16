use std::fs::File;
use std::io::prelude::*;
use std::{fmt, env};

use toml::{self, Parser, Value};

/// Contain the configuration for ElasticSearch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ESConfig {
  pub url:   String,
  pub index: String
}

impl fmt::Display for ESConfig {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "ElasticSearch on {} ({})",
      self.url, self.index)
  }
}

/// Contain instructions about where Searchspot must
/// listen to for new connections.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HTTPConfig {
  pub host: String,
  pub port: u32
}

impl fmt::Display for HTTPConfig {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Listening on http://{}:{}...", self.host, self.port)
  }
}

/// Contain the secrets to grant read and write authorizations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthConfig {
  pub enabled: bool,
  pub read:    String,
  pub write:   String
}

impl fmt::Display for AuthConfig {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Authentication is {}", if self.enabled { "enabled" } else { "disabled" })
  }
}

/// Contain the configuration for the monitor
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MonitorConfig {
  pub provider:     String,
  pub enabled:      bool,
  pub access_token: String,
  pub environment:  String
}

impl fmt::Display for MonitorConfig {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Monitor `{}` is {}", self.provider, if self.enabled { "enabled" } else { "disabled" })
  }
}

/// Container for the configuration structs
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
  pub http:    HTTPConfig,
  pub es:      ESConfig,
  pub auth:    AuthConfig,
  pub monitor: Option<MonitorConfig>
}

impl Config {
  /// Read, parse and return the configuration file
  /// wrapped inside a `Config`. Panic if the file is not
  /// found or cannot be parsed.
  pub fn from_file(path: String) -> Config {
    let mut file = File::open(&path)
        .unwrap_or_else(|err| panic!("Error while reading config file: {}", err));

    let mut config_toml = String::new();
    file.read_to_string(&mut config_toml)
        .unwrap_or_else(|err| panic!("Error while reading config file: {}", err));

    Config::parse(config_toml)
  }

  /// Return a `Config` looking for the parameters
  /// inside the ENV variables. Panic if needed variables
  /// are missing.
  pub fn from_env() -> Config {
    let http_config = HTTPConfig {
      host: env::var("HTTP_HOST").unwrap()
                                 .to_owned(),
      port: env::var("PORT").or(env::var("HTTP_PORT"))
                            .unwrap()
                            .parse::<u32>()
                            .unwrap()
    };

    let es_config = ESConfig {
      url: env::var("ES_URL").unwrap()
                             .to_owned(),
      index: env::var("ES_INDEX").unwrap()
                                 .to_owned()
    };

    let auth_config = AuthConfig {
      enabled: env::var("AUTH_ENABLED").unwrap()
                                       .parse::<bool>()
                                       .unwrap(),
      read: env::var("AUTH_READ").unwrap()
                                 .to_owned(),
      write: env::var("AUTH_WRITE").unwrap()
                                   .to_owned()
    };

    let mut config = Config {
      http:    http_config,
      es:      es_config,
      auth:    auth_config,
      monitor: None
    };

    if let Ok(enabled) = env::var("MONITOR_ENABLED") {
      let monitor_config = MonitorConfig {
        provider: env::var("MONITOR_PROVIDER").unwrap()
                                              .to_owned(),
        enabled: enabled.parse::<bool>()
                        .unwrap(),
        access_token: env::var("MONITOR_ACCESS_TOKEN").unwrap()
                                                      .to_owned(),
        environment: env::var("MONITOR_ENVIRONMENT").unwrap()
                                                    .to_owned()
      };

      config.monitor = Some(monitor_config);
    }

    config
  }

  /// Parse given TOML configuration file and return it
  /// wrapped inside a `Config`.
  pub fn parse(config_toml: String) -> Config {
    let mut parser = Parser::new(&*config_toml);
    let     toml   = parser.parse();

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
    url  = "https://123.0.123.0:9200"
    index = "save_meguka"

    [http]
    host = "1.0.0.127"
    port = 3000

    [auth]
    enabled = true
    read    = "yxxz7oap7rsf67zl"
    write   = "6po2okn3ddwv6ili"

    [monitor]
    provider     = "rollbar"
    enabled      = true
    access_token = "blabla"
    environment  = "test"
  "#;

  #[test]
  fn test_parse() {
    // returns a Config fill with given TOML configuration file
    let config = Config::parse(sample_config.to_owned());
    assert_eq!(config.es.url,    "https://123.0.123.0:9200".to_owned());
    assert_eq!(config.auth.read, "yxxz7oap7rsf67zl".to_owned());
    assert!(config.auth.enabled);

    match config.monitor {
      Some(monitor) => { assert!(monitor.enabled); },
      None          => { assert!(false); }
    };
  }
}
