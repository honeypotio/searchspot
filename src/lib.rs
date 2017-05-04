#![feature(conservative_impl_trait)]

extern crate serde;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate serde_derive;

extern crate iron;
extern crate logger as http_logger;
extern crate router;
extern crate params;
extern crate persistent;
extern crate unicase;

extern crate rs_es;
extern crate toml;
extern crate oath;
extern crate rollbar;
extern crate backtrace;
#[macro_use] extern crate log;
#[macro_use] extern crate maplit;

// this macro is needed by resources/talent.rs
// but moving it to resources/mod raises E0468
#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;

#[macro_use] pub mod macros;

pub mod terms;
pub mod matches;
pub mod config;
pub mod server;
pub mod resource;
pub mod logger;
pub mod monitor;

pub mod resources;
