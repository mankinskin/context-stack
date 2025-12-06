//! Search test case trait and supporting types

use context_trace::{
    tests::test_case::TestCase,
    Token,
};

use crate::Response;

pub trait SearchTestCase: TestCase {
    /// Input query tokens for search
    fn query(&self) -> Vec<Token>;

    /// Expected search response
    fn expected_response(&self) -> Response;
}
