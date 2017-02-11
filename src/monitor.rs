use std::panic::PanicInfo;
use config::MonitorConfig;
use backtrace::Backtrace;

pub struct MonitorProvider;
impl MonitorProvider {
  pub fn find_with_config(monitor: &str, config: &MonitorConfig) -> Option<impl Monitor> {
    match monitor {
      "rollbar" => Some(rollbar::Rollbar::from_config(&config)),
      _         => None
    }
  }

  pub fn null_monitor() -> null_monitor::NullMonitor {
    null_monitor::NullMonitor
  }
}

pub trait Monitor: Send + Sync {
  type MonitorType: Monitor;

  fn from_config(config: &MonitorConfig) -> Self::MonitorType;
  fn send(&self, payload: String);
  fn send_panic(&self, panic_info: &PanicInfo, backtrace: &Backtrace);
  fn is_real(&self) -> bool;
}

mod null_monitor {
  use super::{PanicInfo, Backtrace, Monitor, MonitorConfig};

  pub struct NullMonitor;

  impl Monitor for NullMonitor {
    type MonitorType = NullMonitor;

    fn from_config(_: &MonitorConfig) -> Self::MonitorType {
      NullMonitor
    }

    fn send(&self, _: String) {
      unimplemented!()
    }

    fn send_panic(&self, _: &PanicInfo, _: &Backtrace) {
      unimplemented!()
    }

    fn is_real(&self) -> bool {
      false
    }
  }
}

mod rollbar {
  use super::{PanicInfo, Backtrace, Monitor, MonitorConfig};
  use rollbar::*;

  pub struct Rollbar {
    client: Client
  }

  impl Monitor for Rollbar {
    type MonitorType = Rollbar;

    fn from_config(config: &MonitorConfig) -> Self::MonitorType {
      Rollbar {
        client: Client::new(config.access_token.to_owned(), config.environment.to_owned())
      }
    }

    fn send(&self, payload: String) {
      self.client.send(payload);
    }

    fn send_panic(&self, panic_info: &PanicInfo, backtrace: &Backtrace) {
      self.client.build_report()
        .with_backtrace(&backtrace)
        .from_panic(panic_info)
        .send();
    }

    fn is_real(&self) -> bool {
      true
    }
  }
}
