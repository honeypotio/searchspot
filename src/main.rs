extern crate searchspot;
extern crate backtrace;

use std::{env, panic};

use searchspot::resources::Talent;
use searchspot::server::Server;
use searchspot::config::Config;
use searchspot::monitor::*;
use backtrace::Backtrace;

fn main() {
    let config = match env::args().nth(1) {
      Some(file) => Config::from_file(file),
      None       => Config::from_env()
    };

    if let Some(monitor) = config.monitor.to_owned() {
      if monitor.enabled == true {
        match MonitorProvider::find_with_config(&monitor.provider, &monitor) {
          Some(monitor) => {
            panic::set_hook(Box::new(move |panic_info| {
              let backtrace = Backtrace::new();
              monitor.send_panic(panic_info, &backtrace).join();
            }));
          },
          None => { panic!("Monitor `{}` has not been found.", monitor.provider); }
        };
      }
    }

    let _ = panic::catch_unwind(|| {
      let server = Server::<Talent>::new(config, "/talents");
      server.start();
    });
}
