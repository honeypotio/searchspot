/// Given a `Map`, return a `Vec<_>` that contains all the items
/// wrapped inside the `Value`s.
///
/// Since `iron/params` returns `Result<Map, ParamsError>` (where
/// `Map` is defined as `BTreeMap<String, Value>`) and we're asked to
/// provide `VectorOfTerms<String>::build_terms()` a `Vec<String>`,
/// we need to assert that it `is_ok()` and eventually retrieving
/// its value using the convertion trait `FromValue`.
///
/// Either if the convertion result `is_none()` because of an error
/// or we originally got a `ParamsError`, an empty `Vec<String>` will
/// be returned.
///
/// Otherwise, the output will be a `Vec<String>` fill with all the
/// returned `String`s found inside the query string.
///
/// ```
/// # #[macro_use] extern crate honeysearch;
/// # extern crate params;
/// # use params::*;
///
/// # fn main() {
/// let mut params = Map::new();
/// params.assign("work_roles[]", Value::String("Fullstack".into())).unwrap();
/// params.assign("work_roles[]", Value::String("DevOps".into())).unwrap();
///
/// let work_roles: Vec<String> = vec_from_params!(params, "work_roles");
/// assert_eq!(work_roles, vec!["Fullstack", "DevOps"]);
/// # }
/// ```
///
/// ```
/// # #[macro_use] extern crate honeysearch;
/// # extern crate params;
/// # use params::*;
///
/// # fn main() {
/// let work_roles: Vec<String> = vec_from_params!(Map::new(), "work_roles");
/// assert_eq!(work_roles, Vec::<String>::new());
/// # }
/// ```
#[macro_export]
macro_rules! vec_from_params {
  ($params:expr, $param:expr) => {
    match $params.find(&[$param]) {
      Some(val) => Vec::from_value(val)
                       .unwrap_or(vec![]),
      None => vec![]
    }
  }
}

/// Like vec_from_params!, but all the elements are casted
/// to i32 (or discarded if the type conversion is impossibile).
#[macro_export]
macro_rules! i32_vec_from_params {
  ($params:expr, $param:expr) => {
    match $params.find(&[$param]) {
      Some(company_id) => i32::from_value(company_id)
                              .map(|id| vec![id])
                              .unwrap_or(vec![]),
      None => vec![]
    }
  }
}

#[cfg(test)]
mod tests {
  use params::*;

  #[test]
  fn test_vec_from_params() {
    {
      let mut params = Map::new();
      params.assign("work_roles[]", Value::String("Fullstack".into())).unwrap();
      params.assign("work_roles[]", Value::String("DevOps".into())).unwrap();

      let work_roles: Vec<String> = vec_from_params!(params, "work_roles");
      assert_eq!(work_roles, vec!["Fullstack", "DevOps"]);
    }

    {
      let mut params = Map::new();
      params.assign("work_roles[]", Value::String("".into())).unwrap();

      let work_roles: Vec<String> = vec_from_params!(params, "work_roles");
      assert_eq!(work_roles, vec![""]); // TODO: `vec![]`?
    }

    {
      let work_roles: Vec<String> = vec_from_params!(Map::new(), "work_roles");
      assert_eq!(work_roles, Vec::<String>::new());
    }
  }

  #[test]
  fn test_i32_vec_from_params() {
    {
      let mut params = Map::new();
      params.assign("company_id", Value::String("4".into())).unwrap();

      let company_id: Vec<i32> = i32_vec_from_params!(params, "company_id");
      assert_eq!(company_id, vec![4]);
    }

    {
      let mut params = Map::new();
      params.assign("company_id", Value::String("".into())).unwrap();

      let company_id: Vec<i32> = i32_vec_from_params!(params, "company_id");
      assert_eq!(company_id, vec![]);
    }

    {
      let mut params = Map::new();
      params.assign("company_id", Value::String("madukapls".into())).unwrap();

      let company_id: Vec<i32> = i32_vec_from_params!(params, "company_id");
      assert_eq!(company_id, vec![]);
    }

    {
      let mut params = Map::new();
      params.assign("company_id[]", Value::String("madukapls".into())).unwrap();

      let company_id: Vec<i32> = i32_vec_from_params!(params, "company_id");
      assert_eq!(company_id, vec![]);
    }

    {
      let company_id: Vec<i32> = i32_vec_from_params!(Map::new(), "company_id");
      assert_eq!(company_id, vec![]);
    }
  }
}
