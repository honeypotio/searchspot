#![allow(non_upper_case_globals)]
use rustc_serialize::json::{self, ToJson};

use rs_es::Client;

use iron::prelude::*;
use iron::{status, Handler};
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

pub struct SearchableHandler<R> {
  config:   Config,
  resource: PhantomData<R>,
}

impl<R: Resource> SearchableHandler<R> {
  fn new(config: Config) -> Self {
    SearchableHandler::<R> { resource: PhantomData, config: config }
  }
}

impl<R: Resource> Handler for SearchableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    let mut client = Client::new(&*self.config.es.host, self.config.es.port);

    let params   = try_or_422!(req.get_ref::<Params>());
    let response = SearchResult {
      results: R::search(&mut client, &*self.config.es.index, params),
      params:  params.clone()
    };

    let content_type = "application/json".parse::<Mime>().unwrap();
    Ok(Response::with(
      (content_type, status::Ok, try_or_422!(json::encode(&response.to_json())))
    ))
  }
}

pub struct IndexableHandler<R> {
  config:   Config,
  resource: PhantomData<R>,
}

impl<R: Resource> IndexableHandler<R> {
  fn new(config: Config) -> Self {
    IndexableHandler::<R> { resource: PhantomData, config: config }
  }
}

impl<R: Resource> Handler for IndexableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    let mut client = Client::new(&*self.config.es.host, self.config.es.port);

    let mut payload = String::new();
    req.body.read_to_string(&mut payload).unwrap();

    let resource: R = try_or_422!(json::decode(&payload));
    try_or_422!(resource.index(&mut client, &*self.config.es.index));

    Ok(Response::with(status::Created))
  }
}

pub struct ResettableHandler<R> {
  config:   Config,
  resource: PhantomData<R>,
}

impl<R: Resource> ResettableHandler<R> {
  fn new(config: Config) -> Self {
    ResettableHandler::<R> { resource: PhantomData, config: config }
  }
}

impl<R: Resource> Handler for ResettableHandler<R> {
  fn handle(&self, _: &mut Request) -> IronResult<Response> {
    let mut client = Client::new(&*self.config.es.host, self.config.es.port);

    match R::reset_index(&mut client, &*self.config.es.index) {
      Ok(_)  => Ok(Response::with(status::NoContent)),
      Err(_) => Ok(Response::with(status::UnprocessableEntity))
    }
  }
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
    router.get(&self.endpoint,    SearchableHandler::<R>::new(self.config.clone()));
    router.post(&self.endpoint,   IndexableHandler::<R>::new(self.config.clone()));
    router.delete(&self.endpoint, ResettableHandler::<R>::new(self.config.clone()));

    match env::var("DYNO") { // for some reasons, chain::link makes heroku crash
      Ok(_)  => Iron::new(router).http(&*host),
      Err(_) => {
        let mut chain = Chain::new(router);
        chain.link(Logger::new(None));
        Iron::new(chain).http(&*host)
      }
    }.unwrap();
  }
}
