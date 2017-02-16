use log::*;
use config::Config;
use monitor::*;

// We don't make generic the struct that contains `init`
// to avoid ambiguities in the public interface.
pub struct Logger;

impl Logger {
  pub fn init(config: &Config) -> Result<(), SetLoggerError> {
    set_logger(|_| {
      if let Some(monitor) = config.monitor.to_owned() {
        if monitor.enabled == true {
          match MonitorProvider::find_with_config(&monitor.provider, &monitor) {
            Some(monitor) => { return Box::new(RealLogger { monitor: monitor }); },
            None => { panic!("Monitor {} has not been found.", monitor.provider); }
          };
        }
      }

      Box::new(RealLogger { monitor: MonitorProvider::null_monitor() })
    })
  }
}

pub struct RealLogger<T: Monitor> {
  monitor: T
}

impl<T: Monitor> Log for RealLogger<T> {
  fn enabled(&self, metadata: &LogMetadata) -> bool {
    metadata.level() <= LogLevel::Info
  }

  fn log(&self, record: &LogRecord) {
    if self.enabled(record.metadata()) {
      let error_message = format!("{} - {}", record.level(), record.args());

      if self.monitor.is_real() && record.level() == LogLevel::Error {
        self.monitor.send(&error_message, record.location());
      }

      println!("{} - {:?}", error_message, record.location());
    }
  }
}
