use rustc_serialize::Decodable;

use rs_es::Client;
use rs_es::query::{Filter, Query};
use rs_es::operations::search::Sort;
use rs_es::operations::index::IndexResult;
use rs_es::operations::mapping::*;
use rs_es::error::EsError;

use params::*;

pub trait Resource : Decodable {
  fn search(mut es: &mut Client, default_index: &str, params: &Map) -> Vec<u32>;
  fn index(&self, mut es: &mut Client, index: &str) -> Result<IndexResult, EsError>;
  fn reset_index(mut es: &mut Client, index: &str) -> Result<MappingResult, EsError>;
  fn visibility_filters(epoch: &str, presented_talents: Vec<i32>) -> Vec<Filter>;
  fn search_filters(params: &Map, epoch: &str) -> Query;
  fn sorting_criteria() -> Sort;
}
