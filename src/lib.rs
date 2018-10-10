#![deny(bad_style)]
#![recursion_limit = "128"]

extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

extern crate chrono;
extern crate iron;
extern crate logger as http_logger;
extern crate params;
extern crate persistent;
extern crate router;
extern crate unicase;

extern crate backtrace;
extern crate oath;
extern crate rollbar;
extern crate rs_es;
extern crate toml;
#[macro_use]
extern crate log;

extern crate num_cpus;

#[cfg_attr(test, macro_use)]
#[cfg(test)]
extern crate lazy_static;

#[cfg(test)]
extern crate urlencoded;

#[cfg(test)]
extern crate url;

#[macro_use]
pub mod macros;

pub mod config;
pub mod logger;
pub mod matches;
pub mod monitor;
pub mod resource;
pub mod server;
pub mod terms;

pub mod resources;
