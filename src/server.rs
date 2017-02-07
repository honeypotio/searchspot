#![allow(non_upper_case_globals)]
use serde_json;

use rs_es::Client;

use iron::prelude::*;
use iron::{status, Handler, Headers};
use iron::mime::Mime;

use http_logger::Logger as HTTPLogger;

use router::Router;

use params::*;

use oath::*;

use config::*;
use resource::Resource;
use logger::Logger;

use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::marker::PhantomData;
use std::sync::Mutex;

macro_rules! try_or_422 {
  ($expr:expr) => (match $expr {
    Ok(val)  => val,
    Err(err) => {
      let error_message = err.to_string();
      error!("{}", error_message);

      let mut error = HashMap::new();
      error.insert("error", error_message);

      let content_type = "application/json".parse::<Mime>().unwrap();
      return Ok(Response::with(
        (content_type, status::UnprocessableEntity, serde_json::to_string(&error).unwrap())
      ))
    }
  })
}

macro_rules! unauthorized {
  () => ({
    return Ok(Response::with(
      (status::Unauthorized)
    ))
  })
}

macro_rules! authorization {
  ($trait_name:ident, $mode:ident) => {
    trait $trait_name {
      fn is_authorized(&self, auth_config: AuthConfig, headers: &Headers) -> bool {
        if auth_config.enabled == false {
          return true;
        }

        match headers.get_raw("Authorization") {
          Some(header) => match String::from_utf8(header[0].to_owned()) {
            Ok(header) => {
              match header.split("token ").collect::<Vec<&str>>().last() {
                Some(token) => {
                  match token.parse::<u64>() {
                    Ok(token) => totp_raw(auth_config.$mode.as_bytes(), 6, 0, 30) == token,
                    Err(_)    => false,
                  }
                },
                None => false
              }
            },
            Err(_) => false
          },
          None => false
        }
      }
    }
  }
}

pub struct Server<R: Resource> {
  config:   Config,
  endpoint: String,
  resource: PhantomData<R>
}

authorization!(ReadableEndpoint, read);
authorization!(WritableEndpoint, write);

pub struct SearchableHandler<R> {
  config:   Config,
  client:   Mutex<Client>,
  resource: PhantomData<R>
}

impl<R: Resource> SearchableHandler<R> {
  fn new(config: Config) -> Self {
    SearchableHandler::<R> {
      resource: PhantomData,
      client:   Mutex::new(Client::new(&*config.es.url).unwrap()),
      config:   config
    }
  }
}

impl<R: Resource> ReadableEndpoint for SearchableHandler<R> {}

impl<R: Resource> Handler for SearchableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    if !self.is_authorized(self.config.auth.to_owned(), &req.headers) {
      unauthorized!();
    }

    let params = try_or_422!(req.get_ref::<Params>());

    let response = R::search(&mut self.client.lock().unwrap(), &*self.config.es.index, params);

    let content_type = "application/json".parse::<Mime>().unwrap();
    Ok(Response::with(
      (content_type, status::Ok, try_or_422!(serde_json::to_string(&response)))
    ))
  }
}

pub struct IndexableHandler<R> {
  config:   Config,
  client:   Mutex<Client>,
  resource: PhantomData<R>
}

impl<R: Resource> IndexableHandler<R> {
  fn new(config: Config) -> Self {
    IndexableHandler::<R> {
      resource: PhantomData,
      client:   Mutex::new(Client::new(&*config.es.url).unwrap()),
      config:   config
    }
  }
}

impl<R: Resource> WritableEndpoint for IndexableHandler<R> {}

impl<R: Resource> Handler for IndexableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    if !self.is_authorized(self.config.auth.to_owned(), &req.headers) {
      unauthorized!();
    }

    let mut payload = String::new();
    req.body.read_to_string(&mut payload).unwrap();

    let resources: Vec<R> = try_or_422!(serde_json::from_str(&payload));
    try_or_422!(R::index(&mut self.client.lock().unwrap(), &*self.config.es.index, resources));

    Ok(Response::with(status::Created))
  }
}

pub struct DeletableHandler<R> {
  config:   Config,
  client:   Mutex<Client>,
  resource: PhantomData<R>
}

impl<R: Resource> DeletableHandler<R> {
  fn new(config: Config) -> Self {
    DeletableHandler::<R> {
      resource: PhantomData,
      client:   Mutex::new(Client::new(&*config.es.url).unwrap()),
      config:   config
    }
  }
}

impl<R: Resource> WritableEndpoint for DeletableHandler<R> {}

impl<R: Resource> Handler for DeletableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    if !self.is_authorized(self.config.auth.to_owned(), &req.headers) {
      unauthorized!();
    }

    let ref id = try_or_422!(req.extensions.get::<Router>().unwrap()
                                                           .find("id")
                                                           .ok_or("DELETE#:id not found"));

    match R::delete(&mut self.client.lock().unwrap(), id, &*self.config.es.index) {
      Ok(_)  => Ok(Response::with(status::NoContent)),
      Err(e) => {
        let error_message = e.to_string();
        error!("{}", error_message);

        let content_type = "application/json".parse::<Mime>().unwrap();
        Ok(Response::with(
          (content_type, status::UnprocessableEntity, error_message)
        ))
      }
    }
  }
}

pub struct ResettableHandler<R> {
  config:   Config,
  client:   Mutex<Client>,
  resource: PhantomData<R>
}

impl<R: Resource> ResettableHandler<R> {
  fn new(config: Config) -> Self {
    ResettableHandler::<R> {
      resource: PhantomData,
      client:   Mutex::new(Client::new(&*config.es.url).unwrap()),
      config:   config
    }
  }
}

impl<R: Resource> WritableEndpoint for ResettableHandler<R> {}

impl<R: Resource> Handler for ResettableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    if !self.is_authorized(self.config.auth.to_owned(), &req.headers) {
      unauthorized!();
    }

    match R::reset_index(&mut self.client.lock().unwrap(), &*self.config.es.index) {
      Ok(_)  => Ok(Response::with(status::NoContent)),
      Err(e) => {
        let error_message = e.to_string();
        error!("{}", error_message);

        let content_type = "application/json".parse::<Mime>().unwrap();
        Ok(Response::with(
          (content_type, status::UnprocessableEntity, error_message)
        ))
      }
    }
  }
}

impl<R: Resource> Server<R> {
  pub fn new(config: Config, endpoint: &str) -> Self {
    Server {
      config:   config,
      endpoint: endpoint.to_owned(),
      resource: PhantomData
    }
  }

  pub fn start(&self) {
    Logger::init(&self.config).unwrap();

    let host = format!("{}:{}", self.config.http.host, self.config.http.port);

    println!("Searchspot v{}\n{}\n{}\n", env!("CARGO_PKG_VERSION"),
                                         self.config.es,
                                         self.config.http);

    let mut router = Router::new();
    router.get(&self.endpoint,    SearchableHandler::<R>::new(self.config.to_owned()), "search");
    router.post(&self.endpoint,   IndexableHandler::<R>::new(self.config.to_owned()),  "index");
    router.delete(&self.endpoint, ResettableHandler::<R>::new(self.config.to_owned()), "reset");

    let deletable_endpoint = format!("{}/:id", self.endpoint);
    router.delete(deletable_endpoint, DeletableHandler::<R>::new(self.config.to_owned()), "delete");

    match env::var("DYNO") { // for some reasons, chain::link makes heroku crash
      Ok(_)  => Iron::new(router).http(&*host),
      Err(_) => {
        let mut chain = Chain::new(router);
        chain.link(HTTPLogger::new(None));
        Iron::new(chain).http(&*host)
      }
    }.unwrap();
  }
}

#[cfg(test)]
mod tests {
  use resource::Resource;

  use params::*;

  use rs_es::Client;
  use rs_es::operations::bulk::{BulkResult, Action};
  use rs_es::operations::delete::DeleteResult;
  use rs_es::operations::mapping::{MappingOperation, MappingResult};
  use rs_es::error::EsError;

  #[derive(Serialize, Deserialize, Clone, Debug)]
  pub struct TestResource {
    pub id: u32
  }

  const ES_TYPE: &'static str = "test_resource";

  impl Resource for TestResource {
    type Results = Vec<u32>;

    fn search(_: &mut Client, _: &str, _: &Map) -> Self::Results {
      vec![]
    }

    fn index(mut es: &mut Client, index: &str, resources: Vec<Self>) -> Result<BulkResult, EsError> {
      es.bulk(&resources.into_iter()
                        .map(|r| {
                            let id = r.id.to_string();
                            Action::index(r).with_id(id)
                        })
                        .collect::<Vec<Action<TestResource>>>())
        .with_index(index)
        .with_doc_type(ES_TYPE)
        .send()
    }

    fn delete(mut es: &mut Client, id: &str, index: &str) -> Result<DeleteResult, EsError> {
      es.delete(index, ES_TYPE, id)
        .send()
    }

    fn reset_index(mut es: &mut Client, index: &str) -> Result<MappingResult, EsError> {
      MappingOperation::new(&mut es, index).send()
    }
  }
}
