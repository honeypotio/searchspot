#![cfg_attr(feature = "serde_macros", feature(custom_derive, plugin))]
#![cfg_attr(feature = "serde_macros", plugin(serde_macros))]

extern crate serde;
extern crate serde_json;

extern crate searchspot;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;

#[cfg(feature = "serde_macros")]
include!("main.rs.in");

#[cfg(not(feature = "serde_macros"))]
include!(concat!(env!("OUT_DIR"), "/main.rs"));
