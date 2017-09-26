extern crate chrono;
extern crate rs_es;
extern crate params;

mod talent;
pub use self::talent::Talent;

mod score;
pub use self::score::Score;

#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests {
  extern crate rs_es;
  use self::rs_es::Client;

  use config::*;

  const CONFIG_FILE: &'static str = "examples/tests.toml";

  lazy_static! {
    pub static ref config: Config = Config::from_file(CONFIG_FILE.to_owned());
  }

  pub fn make_client() -> Client {
    Client::new(&*config.es.url).unwrap()
  }

  pub fn refresh_index(client: &mut Client) {
    client.refresh()
          .with_indexes(&[&config.es.index])
          .send()
          .unwrap();
  }
}
