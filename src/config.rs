use toml::{self, Parser, Value};

use std::fs::File;
use std::io::prelude::*;
use std::fmt;
use std::env;

/// Contain the configuration for ElasticSearch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ESConfig {
  pub url:   String,
  pub index: String
}

impl Default for ESConfig {
  fn default() -> ESConfig {
    ESConfig {
      url:  "http://localhost".to_owned(),
      index: "my_index".to_owned()
    }
  }
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

impl Default for HTTPConfig {
  fn default() -> HTTPConfig {
    HTTPConfig {
      host: "127.0.0.1".to_owned(),
      port: 3000
    }
  }
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

impl Default for AuthConfig {
  fn default() -> AuthConfig {
    AuthConfig {
      enabled: false,
      read:    "".to_owned(),
      write:   "".to_owned()
    }
  }
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

impl Default for Config {
  /// Return a new `Config` fill with the default values
  fn default() -> Config {
    Config {
      es:      ESConfig::default(),
      http:    HTTPConfig::default(),
      auth:    AuthConfig::default(),
      monitor: None,
    }
  }
}

impl Config {
  /// Load, parse and return the configuration file
  /// wrapped inside a `Config`.
  pub fn from_file(path: String) -> Config {
    let config_toml = Config::read_file(path).to_owned();
    Config::parse(config_toml)
  }

  /// Return a `Config` looking for the parameters
  /// inside the ENV variables. `panic!` if there
  /// are some missing.
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
      return Config::default();
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
  fn test_new() {
    // returns a Config fill with the default hardcoded data
    let config = Config::default();
    assert_eq!(config.es.url,    "http://localhost".to_owned());
    assert_eq!(config.http.host, "127.0.0.1".to_owned());
    assert_eq!(config.auth.read, "".to_owned());
    assert!(!config.auth.enabled);
    assert!(config.monitor.is_none());
  }

  #[test]
  fn test_parse() {
    // returns a Config fill with given TOML configuration file
    let config = Config::parse(Some(sample_config.to_owned()));
    assert_eq!(config.es.url,    "https://123.0.123.0:9200".to_owned());
    assert_eq!(config.auth.read, "yxxz7oap7rsf67zl".to_owned());
    assert!(config.auth.enabled);

    match config.monitor {
      Some(monitor) => { assert!(monitor.enabled); },
      None          => { assert!(false); }
    };
  }
}
