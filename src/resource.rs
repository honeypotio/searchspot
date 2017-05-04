use serde::de::DeserializeOwned;
use serde::ser::Serialize;

use rs_es::Client;
use rs_es::operations::bulk::BulkResult;
use rs_es::operations::delete::DeleteResult;
use rs_es::operations::mapping::MappingResult;
use rs_es::error::EsError;

use params::*;

use std::any::Any;
use std::fmt::Debug;

pub trait Resource: Send + Sync + Any + Serialize + DeserializeOwned + Debug {
  type Results: Serialize + DeserializeOwned;

  /// Respond to GET requests returning an array with found ids
  fn search(es: &mut Client, default_index: &str, params: &Map) -> Self::Results;

  /// Respond to POST requests indexing given entity
  fn index(es: &mut Client, index: &str, resources: Vec<Self>) -> Result<BulkResult, EsError>;

  /// Respond to DELETE requests on given id deleting it from given index
  fn delete(es: &mut Client, id: &str, index: &str) -> Result<DeleteResult, EsError>;

  /// Respond to DELETE requests rebuilding and reindexing given index
  fn reset_index(es: &mut Client, index: &str) -> Result<MappingResult, EsError>;
}
