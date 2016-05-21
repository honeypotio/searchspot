#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate searchspot;
use searchspot::server::Server;

#[macro_use] pub mod macros;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;

mod resources;
use resources::user::Talent;

fn main() {
  let server = Server::<Talent>::new("/talents");
  server.start();
}
