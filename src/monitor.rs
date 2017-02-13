use std::panic::PanicInfo;
use log::LogLocation;
use config::MonitorConfig;
use backtrace::Backtrace;
use std::thread::JoinHandle;

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
  type ResponseType;

  fn from_config(config: &MonitorConfig) -> Self::MonitorType;
  fn send(&self, error_message: &String, location: &LogLocation);
  fn send_panic(&self, panic_info: &PanicInfo, backtrace: &Backtrace) -> JoinHandle<Self::ResponseType>;
  fn is_real(&self) -> bool;
}

mod null_monitor {
  use super::{PanicInfo, Backtrace, JoinHandle, LogLocation, Monitor, MonitorConfig};

  pub struct NullMonitor;

  impl Monitor for NullMonitor {
    type MonitorType = NullMonitor;
    type ResponseType = Option<()>;

    fn from_config(_: &MonitorConfig) -> Self::MonitorType {
      NullMonitor
    }

    fn send(&self, _: &String, _: &LogLocation) {
      unimplemented!()
    }

    fn send_panic(&self, _: &PanicInfo, _: &Backtrace) -> JoinHandle<Self::ResponseType> {
      unimplemented!()
    }

    fn is_real(&self) -> bool {
      false
    }
  }
}

mod rollbar {
  use super::{PanicInfo, Backtrace, LogLocation, Monitor, MonitorConfig, JoinHandle};
  use rollbar::*;

  pub struct Rollbar {
    client: Client
  }

  impl Monitor for Rollbar {
    type MonitorType = Rollbar;
    type ResponseType = Option<ResponseStatus>;

    fn from_config(config: &MonitorConfig) -> Self::MonitorType {
      Rollbar {
        client: Client::new(config.access_token.to_owned(), config.environment.to_owned())
      }
    }

    fn send(&self, error_message: &String, location: &LogLocation) {
      self.client.build_report()
        .from_error(error_message)
        .with_frame(FrameBuilder::new()
                    .with_line_number(location.line())
                    .with_file_name(location.file())
                    .build())
        .send();
    }

    fn send_panic(&self, panic_info: &PanicInfo, backtrace: &Backtrace) -> JoinHandle<Self::ResponseType> {
      self.client.build_report()
          .from_panic(&panic_info)
          .with_backtrace(&backtrace)
          .send()
    }

    fn is_real(&self) -> bool {
      true
    }
  }
}
