#[cfg(feature = "serde_derive")]
#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

extern crate searchspot;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;

#[cfg(feature = "serde_derive")]
include!("main.rs.in");

#[cfg(not(feature = "serde_derive"))]
include!(concat!(env!("OUT_DIR"), "/main.rs"));
