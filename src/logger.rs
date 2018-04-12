use log::{self, LogRecord, LogMetadata, Log, SetLoggerError, LogLevel, LogLevelFilter};
use config::Config;
use monitor::{Monitor, MonitorProvider};

pub fn start_logging(config: &Config) -> Result<(), SetLoggerError> {
  log::set_logger(|max_log_level| {
    max_log_level.set(LogLevelFilter::Info);

    if let Some(monitor) = config.monitor.to_owned() {
      if monitor.enabled == true {
        match MonitorProvider::find_with_config(&monitor.provider, &monitor) {
          Some(monitor) => { return Box::new(Logger { monitor: monitor }); },
          None          => { panic!("Monitor {} has not been found.", monitor.provider); }
        };
      }
    }

    Box::new(Logger { monitor: MonitorProvider::null_monitor() })
  })
}

struct Logger<T: Monitor> {
  monitor: T
}

impl<T: Monitor> Log for Logger<T> {
  fn enabled(&self, metadata: &LogMetadata) -> bool {
    metadata.level() <= LogLevel::Info
  }

  fn log(&self, record: &LogRecord) {
    if self.enabled(record.metadata()) {
      let error_message = format!("{} - {}", record.level(), record.args());

      if record.level() == LogLevel::Error {
        self.monitor.send(&error_message, record.location());
      }

      println!("{}", error_message);
    }
  }
}
