#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate serde;
extern crate serde_json;
extern crate rs_es;
extern crate iron;
extern crate logger;
extern crate router;
extern crate params;
extern crate toml;
extern crate oath;

#[macro_use] pub mod macros;

pub mod terms;
pub mod config;
pub mod server;
pub mod resource;
