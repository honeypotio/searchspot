macro_rules! vec_from_params {
  ($params:expr, $param:expr) => {
    match $params.find(&[$param]) {
      Some(val) => Vec::from_value(val)
                       .unwrap_or(vec![]),
      None => vec![],
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
      assert_eq!(work_roles, vec![""]); // vec![]?
    }

    {
      let work_roles: Vec<String> = vec_from_params!(Map::new(), "work_roles");
      assert_eq!(work_roles, Vec::<String>::new());
    }
  }
}
