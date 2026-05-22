#[macro_export]
macro_rules! assert_not_indices {
    ($graph:ident, $($name:ident),*) => {
        $(
        let result = $graph.find_ancestor(stringify!($name).chars());
        pretty_assertions::assert_matches!(
            result,
            Err(_) | Ok(_)
        );
        if let Ok(ref response) = result {
            assert!(!response.query_exhausted(),
                "Expected incomplete or error for {}, but got complete match",
                stringify!($name));
        }
        )*
    };
}

#[macro_export]
macro_rules! assert_exhausted_indices {
    ($graph:ident, $($name:ident),*) => {
        $(
        let response = $graph
            .find_ancestor(stringify!($name).chars())
            .unwrap_or_else(|e| panic!(
                "Failed to find index for {}: {:?}",
                stringify!($name),
                e
            ));
        assert!(
            response.query_exhausted(),
            "Expected exhausted match for {}, but query was not exhausted: {:#?}",
            stringify!($name),
            response
        );
        let $name = response.root_token();
        )*
    };
}
