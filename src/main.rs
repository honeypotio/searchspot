extern crate searchspot;

use searchspot::resources::Talent;
use searchspot::server::Server;

fn main() {
  let server = Server::<Talent>::new("/talents");
  server.start();
}
