//! Insert test case trait and supporting types

use context_trace::{
    Token,
    graph::vertex::data::VertexData,
    tests::test_case::{
        TestCase,
        TestEnv,
    },
};

/// Test case for insert operations.
///
/// Validates that insertion produces correct token structure,
/// pattern relationships, and graph state.
///
pub trait InsertTestCase: TestCase {
    /// Get a fresh environment instance
    fn environment(&self) -> Self::Env {
        Self::Env::initialize()
    }

    /// Input tokens to insert
    fn input_tokens(&self) -> Vec<Token>;

    /// Expected resulting token after insertion
    fn expected_token(&self) -> Token;

    /// Expected string representation of the token
    fn expected_string(&self) -> &str;

    /// Expected vertex data after insertion
    fn expected_vertex_data(&self) -> VertexData;
}
