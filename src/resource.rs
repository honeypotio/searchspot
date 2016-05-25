use serde::de::Deserialize;

use rs_es::Client;
use rs_es::operations::index::IndexResult;
use rs_es::operations::mapping::*;
use rs_es::error::EsError;

use params::*;

use std::any::Any;
use std::fmt::Debug;

pub trait Resource : Send + Sync + Any + Deserialize + Debug {
  /// Respond to GET requests returning an array with found ids
  fn search(mut es: &mut Client, default_index: &str, params: &Map) -> Vec<u32>;

  /// Respond to POST requests indexing given entity
  fn index(&self, mut es: &mut Client, index: &str) -> Result<IndexResult, EsError>;

  /// Respond to DELETE requests deleting all the entities from given index
  fn reset_index(mut es: &mut Client, index: &str) -> Result<MappingResult, EsError>;
}
