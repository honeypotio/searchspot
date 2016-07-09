#![cfg_attr(feature = "serde_macros", feature(custom_derive, plugin))]
#![cfg_attr(feature = "serde_macros", plugin(serde_macros))]

extern crate serde;
extern crate serde_json;
extern crate rs_es;
extern crate iron;
extern crate logger as http_logger;
extern crate router;
extern crate params;
extern crate toml;
extern crate oath;
#[macro_use] extern crate log;

#[cfg(feature = "serde_macros")]
include!("lib.rs.in");

#[cfg(not(feature = "serde_macros"))]
include!(concat!(env!("OUT_DIR"), "/lib.rs"));
