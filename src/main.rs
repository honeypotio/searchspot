extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;

extern crate searchspot;

#[macro_use] pub mod macros;
mod resources;
use resources::Talent;

use searchspot::server::Server;

fn main() {
  let server = Server::<Talent>::new("/talents");
  server.start();
}
