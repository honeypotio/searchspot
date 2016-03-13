extern crate rustc_serialize;
extern crate rs_es;
extern crate chrono;

extern crate iron;
extern crate logger;
extern crate router;
extern crate params;

#[macro_use] pub mod macros;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;

extern crate searchspot;
use searchspot::server::Server;

mod resources;
use resources::user::Talent;

fn main() {
  let server = Server::<Talent>::new("/talents".to_owned());
  server.start();
}
