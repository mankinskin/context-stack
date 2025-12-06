//! Insert test case trait and supporting types

use context_trace::{
    PatternId,
    Token,
    tests::test_case::{TestCase, TestEnv},
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

// Note: The actual validation logic will be implemented once we integrate with context-insert.
// The validate() method will look like:
//
// fn validate(&self) -> Result<(), TestError> {
//     use context_insert::insert::ToInsertCtx;
//
//     let env = self.environment();
//     let input = self.input_tokens();
//     let expected_token = self.expected_token();
//     let expected_string = self.expected_string();
//     let expected_patterns = self.expected_patterns();
//
//     // Execute insert
//     let actual_token = env.graph()
//         .insert(input.clone())
//         .map_err(|e| TestError::InsertFailed(format!("{:?}", e)))?;
//
//     // Validate token
//     if actual_token != expected_token {
//         return Err(TestError::TokenMismatch {
//             test_case: self.name(),
//             expected: format!("{:?}", expected_token),
//             actual: format!("{:?}", actual_token),
//         });
//     }
//
//     // Validate string representation
//     let g = env.graph().graph();
//     let actual_string = g.expect_vertex(actual_token).to_string();
//     if actual_string != expected_string {
//         return Err(TestError::TokenMismatch {
//             test_case: self.name(),
//             expected: expected_string.to_string(),
//             actual: actual_string,
//         });
//     }
//
//     // Validate patterns
//     for expected_pat in expected_patterns {
//         let vertex = g.expect_vertex(expected_pat.token);
//         // ... validate pattern structure
//     }
//
//     Ok(())
// }
