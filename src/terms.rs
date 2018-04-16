use rs_es::query::Query;

pub trait VectorOfTerms<T> {
    /// Extract the elements inside `Vec<T>` into `Vec<Filter>`, if present.
    /// Every element will be mapped into a `JsonVal`.
    fn build_terms(key: &str, values: &Vec<T>) -> Vec<Query>;
}

impl VectorOfTerms<String> for Query {
    fn build_terms(key: &str, values: &Vec<String>) -> Vec<Query> {
        if values.is_empty() {
            return vec![];
        }

        vec![
            Query::build_terms(key)
                .with_values(values.iter().map(AsRef::as_ref).collect::<Vec<&str>>())
                .build(),
        ]
    }
}

macro_rules! build_vector_of_terms_impl {
    ($t:ty) => {
        impl<'a> VectorOfTerms<$t> for Query {
            fn build_terms(key: &str, values: &Vec<$t>) -> Vec<Query> {
                if values.is_empty() {
                    return vec![];
                }

                vec![
                    Query::build_terms(key)
                        .with_values(values.to_owned())
                        .build(),
                ]
            }
        }
    };
}

build_vector_of_terms_impl!(i32);

#[cfg(test)]
mod tests {
    use super::*;
    use rs_es::query::Query;
    use serde_json;

    #[test]
    fn test_vector_of_terms() {
        assert!(<Query as VectorOfTerms<String>>::build_terms("work_roles", &vec![]).is_empty());

        {
            let filters = <Query as VectorOfTerms<String>>::build_terms(
                "work_roles",
                &vec!["Fullstack".to_owned()],
            );
            assert_eq!(
                serde_json::to_string(&filters[0]).unwrap(),
                "{\"terms\":{\"work_roles\":[\"Fullstack\"]}}".to_owned()
            );
        }

        {
            let filters = <Query as VectorOfTerms<i32>>::build_terms("work_roles", &vec![1]);
            assert_eq!(
                serde_json::to_string(&filters[0]).unwrap(),
                "{\"terms\":{\"work_roles\":[1]}}".to_owned()
            );
        }
    }
}
