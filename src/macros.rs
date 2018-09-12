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
/// # #[macro_use] extern crate searchspot;
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
/// # #[macro_use] extern crate searchspot;
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
        match $params.get($param) {
            Some(val) => Vec::from_value(val).unwrap_or(vec![]),
            None => vec![],
        }
    };
}

#[macro_export]
macro_rules! vec_from_maybe_csv_params {
    ($params:expr, $param:expr) => {
        match $params.get($param) {
            Some(val @ Value::Array(_)) => Vec::from_value(val).unwrap_or(vec![]),
            Some(Value::String(csv)) => csv.split(',').flat_map(|v| v.trim().parse().ok()).collect::<Vec<_>>(),
            _ => vec![],
        }
    };
}

/// Like `vec_from_params`, but expects `$t` (instead of `Vec<$t>`)
/// and return `Vec<$t>`. Elements that cannot be actually
/// casted to `$t` are discarded.
#[macro_export]
macro_rules! type_vec_from_params {
    ($t:ident, $params:expr, $param:expr) => {
        match $params.get($param) {
            Some(val) => $t::from_value(val).map(|id| vec![id]).unwrap_or(vec![]),
            None => vec![],
        }
    };
}

/// Sugar for `type_vec_from_params` where `$t` is `String`.
#[macro_export]
macro_rules! string_vec_from_params {
    ($params:expr, $param:expr) => {
        type_vec_from_params!(String, $params, $param)
    };
}

/// Sugar for `type_vec_from_params` where `$t` is `i32`.
#[macro_export]
macro_rules! i32_vec_from_params {
    ($params:expr, $param:expr) => {
        type_vec_from_params!(i32, $params, $param)
    };
}

#[cfg(test)]
mod tests {
    use params::{FromValue, Map, Value};

    #[test]
    fn test_vec_from_params() {
        // given to strings, it returns a vector containing given strings
        {
            let mut params = Map::new();
            params
                .assign("work_roles[]", Value::String("Fullstack".into()))
                .unwrap();
            params
                .assign("work_roles[]", Value::String("DevOps".into()))
                .unwrap();

            let work_roles: Vec<String> = vec_from_params!(params, "work_roles");
            assert_eq!(work_roles, vec!["Fullstack", "DevOps"]);
        }

        // given an empty string, it returns a vector containing an empty strings
        {
            let mut params = Map::new();
            params
                .assign("work_roles[]", Value::String("".into()))
                .unwrap();

            let work_roles: Vec<String> = vec_from_params!(params, "work_roles");
            assert_eq!(work_roles, vec![""]); // TODO: `vec![]`?
        }

        // given nothing, it returns an empty vector
        {
            let work_roles: Vec<String> = vec_from_params!(Map::new(), "work_roles");
            assert_eq!(work_roles, Vec::<String>::new());
        }
    }

    #[test]
    fn test_i32_vec_from_params() {
        // given a number casted to String, it returns a vector containing that string casted to i32
        {
            let mut params = Map::new();
            params
                .assign("company_id", Value::String("4".into()))
                .unwrap();

            let company_ids: Vec<i32> = i32_vec_from_params!(params, "company_id");
            assert_eq!(company_ids, vec![4]);
        }

        // given an empty string, it returns an empty vector
        {
            let mut params = Map::new();
            params
                .assign("company_id", Value::String("".into()))
                .unwrap();

            let company_ids: Vec<i32> = i32_vec_from_params!(params, "company_id");
            assert!(company_ids.is_empty());
        }

        // given a non-number string, it returns an empty vector
        {
            let mut params = Map::new();
            params
                .assign("company_id", Value::String("madukapls".into()))
                .unwrap();

            let company_ids: Vec<i32> = i32_vec_from_params!(params, "company_id");
            assert!(company_ids.is_empty());
        }

        {
            let mut params = Map::new();
            params
                .assign("company_id[]", Value::String("madukapls".into()))
                .unwrap();

            let company_ids: Vec<i32> = i32_vec_from_params!(params, "company_id");
            assert!(company_ids.is_empty());
        }

        // given nothing, it returns an empty vector
        {
            let company_ids: Vec<i32> = i32_vec_from_params!(Map::new(), "company_id");
            assert!(company_ids.is_empty());
        }
    }
}
