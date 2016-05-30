use log::*;

pub struct Logger;

impl Logger {
  pub fn init() -> Result<(), SetLoggerError> {
    set_logger(|max_log_level| {
      max_log_level.set(LogLevelFilter::Info);
      Box::new(Logger)
    })
  }
}

impl Log for Logger {
  fn enabled(&self, metadata: &LogMetadata) -> bool {
    metadata.level() <= LogLevel::Info
  }

  fn log(&self, record: &LogRecord) {
    if self.enabled(record.metadata()) {
      println!("{} - {}", record.level(), record.args());
    }
  }
}
