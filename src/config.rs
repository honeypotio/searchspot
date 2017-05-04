use std::fs::File;
use std::io::prelude::*;
use std::{fmt, env};

use toml;

/// Contain the configuration for ElasticSearch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ES {
  pub url:   String,
  pub index: String
}

impl fmt::Display for ES {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "ElasticSearch on {} ({})", self.url, self.index)
  }
}

/// Contain instructions about where Searchspot must
/// listen to for new connections.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HTTP {
  pub host: String,
  pub port: u32
}

impl fmt::Display for HTTP {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Listening on http://{}:{}...", self.host, self.port)
  }
}

/// Contain the secrets to grant read and write authorizations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Auth {
  pub enabled: bool,
  pub read:    String,
  pub write:   String
}

impl fmt::Display for Auth {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Authentication is {}.", if self.enabled { "enabled" } else { "disabled" })
  }
}

/// Contain the configuration for the monitor.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Monitor {
  pub provider:     String,
  pub enabled:      bool,
  pub access_token: String,
  pub environment:  String
}

impl fmt::Display for Monitor {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Monitor `{}` is {}.", self.provider, if self.enabled { "enabled" } else { "disabled" })
  }
}

/// Contain the configuration for the tokens.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Tokens {
  pub lifetime: TokensLifetime
}

impl fmt::Display for Tokens {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.lifetime)
  }
}

/// Contain the configuration for the token lifetimes.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokensLifetime {
  pub read:  u64,
  pub write: u64
}

impl fmt::Display for TokensLifetime {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Each read token will have a lifetime of {}s. Each write token will have a lifetime of {}s.", self.read, self.write)
  }
}

impl Default for TokensLifetime {
  fn default() -> TokensLifetime {
    TokensLifetime {
      read:  30,
      write: 30
    }
  }
}

/// Container for the configuration structs
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
  pub http:    HTTP,
  pub es:      ES,
  pub auth:    Auth,
  #[serde(default)]
  pub tokens:  Tokens,
  pub monitor: Option<Monitor>
}

impl Config {
  /// Read, parse and return the configuration file
  /// wrapped inside a `Config`. Panic if the file is not
  /// found or cannot be parsed.
  pub fn from_file(path: String) -> Config {
    let mut file = File::open(&path)
        .unwrap_or_else(|err| panic!("Error while reading config file: {}", err));

    let mut toml = String::new();
    file.read_to_string(&mut toml)
        .unwrap_or_else(|err| panic!("Error while reading config file: {}", err));

    Config::parse(&toml)
  }

  /// Return a `Config` looking for the parameters
  /// inside the ENV variables. Panic if needed variables
  /// are missing.
  pub fn from_env() -> Config {
    // this stuff should be performed by serde, but the naming conventions used by
    // the config file and the environment vars are different...
    let http = HTTP {
      host: env::var("HTTP_HOST").unwrap().to_owned(),
      port: env::var("PORT").or(env::var("HTTP_PORT")).unwrap()
                            .parse().unwrap()
    };

    let es = ES {
      url:   env::var("ES_URL").unwrap().to_owned(),
      index: env::var("ES_INDEX").unwrap().to_owned()
    };

    let auth = Auth {
      enabled: env::var("AUTH_ENABLED").unwrap()
                                       .parse().unwrap(),
      read:  env::var("AUTH_READ").unwrap().to_owned(),
      write: env::var("AUTH_WRITE").unwrap().to_owned()
    };

    let tokens = Tokens {
      lifetime: TokensLifetime {
        read:  env::var("TOKEN_READ_LIFETIME").map(|t| t.parse().unwrap()).unwrap_or(30),
        write: env::var("TOKEN_WRITE_LIFETIME").map(|t| t.parse().unwrap()).unwrap_or(30)
      }
    };

    let monitor = if let Ok(enabled) = env::var("MONITOR_ENABLED") {
      Some(Monitor {
        provider: env::var("MONITOR_PROVIDER").unwrap().to_owned(),
        enabled:  enabled.parse().unwrap(),
        access_token: env::var("MONITOR_ACCESS_TOKEN").unwrap().to_owned(),
        environment:  env::var("MONITOR_ENVIRONMENT").unwrap().to_owned()
      })
    }
    else {
      None
    };

    Config {
      http:    http,
      es:      es,
      auth:    auth,
      tokens:  tokens,
      monitor: monitor
    }
  }

  /// Parse given TOML configuration file and return it
  /// wrapped inside a `Config`.
  pub fn parse(toml: &str) -> Config {
    match toml::from_str(toml) {
      Ok(config) => config,
      Err(error) => {
        println!("{:?}", error);
        panic!("Error while parsing the configuration file.");
      }
    }
  }
}

impl fmt::Display for Config {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let monitor = match self.monitor {
      Some(ref monitor) => format!("{}", monitor),
      None              => "No monitor has been configured.".to_owned()
    };

    write!(f, "{}\n{}\n{}\n{}\n{}", self.auth, self.tokens, monitor, self.es, self.http)
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

    [tokens]
    [tokens.lifetime]
    read  = 30
    write = 99
  "#;

  #[test]
  fn test_parse() {
    // returns a Config fill with given TOML configuration file
    let config = Config::parse(&sample_config);
    assert_eq!(config.es.url,    "https://123.0.123.0:9200".to_owned());
    assert_eq!(config.auth.read, "yxxz7oap7rsf67zl".to_owned());
    assert!(config.auth.enabled);
    assert!(config.monitor.unwrap().enabled);
    assert_eq!(config.tokens.lifetime.write, 99);
  }
}
