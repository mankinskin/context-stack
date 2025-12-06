//! Insert test case trait and supporting types

use context_trace::{
    PatternId,
    Token,
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

    /// Expected pattern structures
    fn expected_patterns(&self) -> Vec<ExpectedPattern>;
}

/// Expected pattern structure for validation
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedPattern {
    /// The token this pattern produces
    pub token: Token,

    /// The structure of the pattern (sequence of tokens)
    pub structure: Vec<Token>,

    /// Pattern IDs associated with this structure
    pub pattern_ids: Vec<PatternId>,
}

impl ExpectedPattern {
    /// Create a new expected pattern
    pub fn new(
        token: Token,
        structure: Vec<Token>,
        pattern_ids: Vec<PatternId>,
    ) -> Self {
        Self {
            token,
            structure,
            pattern_ids,
        }
    }
}
