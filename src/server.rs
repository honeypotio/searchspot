#![allow(non_upper_case_globals)]
use rustc_serialize::json::{self, ToJson};

use rs_es::Client;

use iron::prelude::*;
use iron::status;
use iron::mime::Mime;

use logger::Logger;

use router::Router;

use params::*;

use config::*;
use search::SearchResult;
use resource::Resource;

use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::marker::PhantomData;

macro_rules! try_or_422 {
  ($expr:expr) => (match $expr {
    Ok(val)  => val,
    Err(err) => {
      let mut error = HashMap::new();
      error.insert("error", format!("{}", err));

      let content_type = "application/json".parse::<Mime>().unwrap();
      return Ok(Response::with(
        (content_type, status::UnprocessableEntity, json::encode(&error).unwrap())
      ))
    }
  })
}

#[derive(Clone)]
pub struct Server<R: Resource> {
  config:   Config,
  endpoint: String,
  resource: PhantomData<R>
}

impl<R: Resource> Server<R> {
  pub fn new(endpoint: String) -> Self {
    let config = match env::args().nth(1) {
      Some(file) => Config::from_file(file),
      None       => Config::from_env()
    };

    Server {
      config:   config,
      endpoint: endpoint,
      resource: PhantomData
    }
  }

  pub fn start(&self) {
    let host = format!("{}:{}", self.config.http.host, self.config.http.port);

    println!("Searchspot v{}\n{}\n{}\n", env!("CARGO_PKG_VERSION"),
                                         self.config.es,
                                         self.config.http);

    let mut router = Router::new();
    self.handle_search(&mut router);
    self.handle_indexing(&mut router);
    self.handle_reset(&mut router);

    let mut chain = Chain::new(router);

    // for some reasons, chain::link makes heroku crash
    if env::var("DYNO").is_err() {
      chain.link(Logger::new(None));
    }

    Iron::new(chain).http(&*host).unwrap();
  }

  fn handle_search(&self, mut router: &mut Router) {
    let config = self.config.clone();

    router.get(self.endpoint.clone(), move |r: &mut Request| {
      let mut es = Client::new(&*config.es.host, config.es.port);
      Self::search(r, &mut es, &*config.es.index)
    });
  }

  fn handle_indexing(&self, mut router: &mut Router) {
    let config = self.config.clone();

    router.post(self.endpoint.clone(), move |r: &mut Request| {
      let mut es = Client::new(&*config.es.host, config.es.port);
      Self::index(r, &mut es, &*config.es.index)
    });
  }

  fn handle_reset(&self, mut router: &mut Router) {
    let config = self.config.clone();

    router.delete(self.endpoint.clone(), move |r: &mut Request| {
      let mut es = Client::new(&*config.es.host, config.es.port);
      Self::reset(r, &mut es, &*config.es.index)
    });
  }

  fn search(req: &mut Request, mut es: &mut Client, index: &str) -> IronResult<Response> {
    let params   = try_or_422!(req.get_ref::<Params>());
    let response = SearchResult {
      results: R::search(&mut es, index, params),
      params:  params.clone()
    };

    let content_type = "application/json".parse::<Mime>().unwrap();
    Ok(Response::with(
      (content_type, status::Ok, try_or_422!(json::encode(&response.to_json())))
    ))
  }

  fn index(req: &mut Request, mut es: &mut Client, index: &str) -> IronResult<Response> {
    let mut payload = String::new();
    req.body.read_to_string(&mut payload).unwrap();

    let resource: R = try_or_422!(json::decode(&payload));
    try_or_422!(resource.index(es, index));

    Ok(Response::with(status::Created))
  }

  fn reset(_: &mut Request, mut es: &mut Client, index: &str) -> IronResult<Response> {
    match R::reset_index(&mut es, index) {
      Ok(_)  => Ok(Response::with(status::NoContent)),
      Err(_) => Ok(Response::with(status::UnprocessableEntity))
    }
  }
}
