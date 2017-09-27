#![allow(non_upper_case_globals)]
use serde_json;

use rs_es::Client;

use iron::prelude::*;
use iron::{status, Handler, Headers};
use iron::mime::Mime;
use iron::typemap::Key;
use iron::headers;
use iron::middleware::AfterMiddleware;
use iron::method::Method::*;
use unicase::UniCase;

use persistent::Write;

use http_logger::Logger as HTTPLogger;

use router::Router;

use params::*;

use oath::{totp_raw_now, HashType};

use config::Auth as AuthConfig;
use config::Config;

use resource::Resource;
use logger::start_logging;

use std::collections::HashMap;
use std::io::Read;
use std::marker::PhantomData;

#[derive(Copy, Clone)]
pub struct SharedClient;

impl Key for SharedClient { type Value = Client; }

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
      fn is_authorized(&self, auth_config: &AuthConfig, headers: &Headers, token_lifetime: u64) -> bool {
        if auth_config.enabled == false {
          return true;
        }

        match headers.get_raw("Authorization") {
          Some(header) => match String::from_utf8(header[0].to_owned()) {
            Ok(header) => {
              match header.split("token ").collect::<Vec<&str>>().last() {
                Some(token) => {
                  match token.parse::<u64>() {
                    Ok(token) => totp_raw_now(auth_config.$mode.as_bytes(), 6, 0, token_lifetime as u64, &HashType::SHA1) == token,
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

authorization!(ReadableEndpoint, read);
authorization!(WritableEndpoint, write);

pub struct Server {
  config: Config
}

pub struct SearchableHandler<R> {
  config:   Config,
  resource: PhantomData<R>
}

impl<R: Resource> SearchableHandler<R> {
  pub fn new(config: Config) -> Self {
    SearchableHandler::<R> {
      resource: PhantomData,
      config:   config
    }
  }
}

impl<R: Resource> ReadableEndpoint for SearchableHandler<R> {}

impl<R: Resource> Handler for SearchableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    let ref lifetimes = self.config.tokens.lifetime;
    if !self.is_authorized(&self.config.auth, &req.headers, lifetimes.read) {
      unauthorized!();
    }

    let client = req.get::<Write<SharedClient>>().unwrap();
    let params = try_or_422!(req.get_ref::<Params>());

    let response = R::search(&mut client.lock().unwrap(), &*self.config.es.index, params);

    let content_type = "application/json".parse::<Mime>().unwrap();
    Ok(Response::with(
      (content_type, status::Ok, try_or_422!(serde_json::to_string(&response)))
    ))
  }
}

pub struct IndexableHandler<R> {
  config:   Config,
  resource: PhantomData<R>
}

impl<R: Resource> IndexableHandler<R> {
  pub fn new(config: Config) -> Self {
    IndexableHandler::<R> {
      resource: PhantomData,
      config:   config
    }
  }
}

impl<R: Resource> WritableEndpoint for IndexableHandler<R> {}

impl<R: Resource> Handler for IndexableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    let ref lifetimes = self.config.tokens.lifetime;
    if !self.is_authorized(&self.config.auth, &req.headers, lifetimes.write) {
      unauthorized!();
    }

    let mut payload = String::new();
    req.body.read_to_string(&mut payload).unwrap();

    let resources: Vec<R> = try_or_422!(serde_json::from_str(&payload));
    let client = req.get::<Write<SharedClient>>().unwrap();
    try_or_422!(R::index(&mut client.lock().unwrap(), &*self.config.es.index, resources));

    Ok(Response::with(status::Created))
  }
}

pub struct DeletableHandler<R> {
  config:   Config,
  resource: PhantomData<R>
}

impl<R: Resource> DeletableHandler<R> {
  pub fn new(config: Config) -> Self {
    DeletableHandler::<R> {
      resource: PhantomData,
      config:   config
    }
  }
}

impl<R: Resource> WritableEndpoint for DeletableHandler<R> {}

impl<R: Resource> Handler for DeletableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    let ref lifetimes = self.config.tokens.lifetime;
    if !self.is_authorized(&self.config.auth, &req.headers, lifetimes.write) {
      unauthorized!();
    }

    let     client = req.get::<Write<SharedClient>>().unwrap();
    let mut client = client.lock().unwrap();

    let ref id = try_or_422!(req.extensions.get::<Router>().unwrap()
                                                           .find("id")
                                                           .ok_or("DELETE#:id not found"));

    match R::delete(&mut client, id, &*self.config.es.index) {
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
  resource: PhantomData<R>
}

impl<R: Resource> ResettableHandler<R> {
  pub fn new(config: Config) -> Self {
    ResettableHandler::<R> {
      resource: PhantomData,
      config:   config
    }
  }
}

impl<R: Resource> WritableEndpoint for ResettableHandler<R> {}

impl<R: Resource> Handler for ResettableHandler<R> {
  fn handle(&self, req: &mut Request) -> IronResult<Response> {
    let ref lifetimes = self.config.tokens.lifetime;
    if !self.is_authorized(&self.config.auth, &req.headers, lifetimes.write) {
      unauthorized!();
    }

    let     client = req.get::<Write<SharedClient>>().unwrap();
    let mut client = client.lock().unwrap();
    match R::reset_index(&mut client, &*self.config.es.index) {
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

struct CorsMiddleware;

impl AfterMiddleware for CorsMiddleware {
  fn after(&self, _: &mut Request, mut res: Response) -> IronResult<Response> {
    res.headers.set(headers::AccessControlAllowOrigin::Any);
    res.headers.set(headers::AccessControlAllowHeaders(vec![
      UniCase("x-requested-withcontent-type".to_owned()),
      UniCase("content-type".to_owned()),
      UniCase("accept".to_owned()),
      UniCase("authorization".to_owned())
    ]));
    res.headers.set(headers::AccessControlAllowMethods(vec![Get, Post, Put, Delete]));
    Ok(res)
  }
}

impl Server {
  pub fn new(config: Config) -> Self {
    Server { config: config }
  }

  pub fn start(&self, router: Router) {
    start_logging(&self.config).unwrap();

    let host = format!("{}:{}", self.config.http.host, self.config.http.port);

    println!("Searchspot v{}\n{}\n", env!("CARGO_PKG_VERSION"), self.config);

    let client = Client::new(&*self.config.to_owned().es.url).unwrap();

    let mut chain = Chain::new(router);
    chain.link(Write::<SharedClient>::both(client));
    chain.link(HTTPLogger::new(None));
    chain.link_after(CorsMiddleware);
    Iron::new(chain).http(&*host).unwrap();
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

    fn index(es: &mut Client, index: &str, resources: Vec<Self>) -> Result<BulkResult, EsError> {
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

    fn delete(es: &mut Client, id: &str, index: &str) -> Result<DeleteResult, EsError> {
      es.delete(index, ES_TYPE, id)
        .send()
    }

    fn reset_index(mut es: &mut Client, index: &str) -> Result<MappingResult, EsError> {
      MappingOperation::new(&mut es, index).send()
    }
  }
}
