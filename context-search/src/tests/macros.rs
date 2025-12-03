#[macro_export]
macro_rules! assert_not_indices {
    ($graph:ident, $($name:ident),*) => {
        $(
        let result = $graph.find_ancestor(stringify!($name).chars());
        assert_matches!(
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
