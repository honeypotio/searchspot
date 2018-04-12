extern crate backtrace;
extern crate searchspot;
#[macro_use]
extern crate router;

use backtrace::Backtrace;
use searchspot::config::Config;
use searchspot::monitor::{Monitor, MonitorProvider};
use searchspot::resources::{Score, Talent};
use searchspot::server::Server;
use searchspot::server::{DeletableHandler, IndexableHandler, ResettableHandler, SearchableHandler};
use std::{env, panic};

fn main() {
    let config = match env::args().nth(1) {
        Some(file) => Config::from_file(file),
        None => Config::from_env(),
    };

    if let Some(monitor) = config.monitor.to_owned() {
        if monitor.enabled == true {
            match MonitorProvider::find_with_config(&monitor.provider, &monitor) {
                Some(monitor) => {
                    panic::set_hook(Box::new(move |panic_info| {
                        let backtrace = Backtrace::new();
                        let _ = monitor.send_panic(panic_info, &backtrace).join();
                    }));
                }
                None => {
                    panic!("Monitor `{}` has not been found.", monitor.provider);
                }
            };
        }
    }

    let _ = panic::catch_unwind(|| {
        let server = Server::new(config.to_owned());

        let router = router!{
          get_talents:    get    "/talents" => SearchableHandler::<Talent>::new(config.to_owned()),
          create_talents: post   "/talents" => IndexableHandler::<Talent>::new(config.to_owned()),
          delete_talents: delete "/talents" => ResettableHandler::<Talent>::new(config.to_owned()),
          delete_talent:  delete "/talents/:id" => DeletableHandler::<Talent>::new(config.to_owned()),

          create_scores: post "/scores" => IndexableHandler::<Score>::new(config.to_owned()),
        };

        server.start(router);
    });
}
