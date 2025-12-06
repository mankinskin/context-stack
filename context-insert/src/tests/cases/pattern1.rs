//! Insert test case for index_pattern1 environment
//!
//! Tests inserting patterns with overlapping structures.

use crate::tests::{
    env::EnvIndexPattern1,
    test_case::InsertTestCase,
};
use context_trace::{
    graph::vertex::{
        VertexIndex,
        data::VertexData,
        pattern::Pattern,
    },
    tests::test_case::{TestCase, TestEnv},
    PatternId,
    Token,
};

/// Test case: Insert "byz" into pattern1 environment
pub struct Pattern1Byz;

impl TestCase for Pattern1Byz {
    type Env = EnvIndexPattern1;

    fn name(&self) -> &'static str {
        "index_pattern1_insert_byz"
    }
}

impl InsertTestCase for Pattern1Byz {
    fn input_tokens(&self) -> Vec<Token> {
        let env = <Self as TestCase>::Env::get();
        vec![env.by, env.z]
    }

    fn expected_token(&self) -> Token {
        let env = <Self as TestCase>::Env::get();
        // The token will be created during insertion, but we can describe it
        // This is a placeholder - in practice, we'd compute it or get it from env after insertion
        Token::new(VertexIndex(0), 3)
    }

    fn expected_string(&self) -> &str {
        "byz"
    }

    fn expected_vertex_data(&self) -> VertexData {
        let env = <Self as TestCase>::Env::get();
        let token = self.expected_token();
        let mut vertex = VertexData::new(token);
        
        // Add expected child pattern
        let pattern = Pattern::from(vec![env.by, env.z]);
        let pattern_id = PatternId::default();
        vertex.child_patterns_mut().insert(pattern_id, pattern);
        
        vertex
    }
}

/// Test case: Insert "aby" into pattern1 environment
pub struct Pattern1Aby;

impl TestCase for Pattern1Aby {
    type Env = EnvIndexPattern1;

    fn name(&self) -> &'static str {
        "index_pattern1_insert_aby"
    }
}
impl InsertTestCase for Pattern1Aby {
    fn input_tokens(&self) -> Vec<Token> {
        let env = <Self as TestCase>::Env::get();
        vec![env.ab, env.y]
    }

    fn expected_token(&self) -> Token {
        Token::new(VertexIndex(0), 3)
    }

    fn expected_string(&self) -> &str {
        "aby"
    }

    fn expected_vertex_data(&self) -> VertexData {
        let env = <Self as TestCase>::Env::get();
        let token = self.expected_token();
        let mut vertex = VertexData::new(token);
        
        // Add expected child pattern
        let pattern = Pattern::from(vec![env.ab, env.y]);
        let pattern_id = PatternId::default();
        vertex.child_patterns_mut().insert(pattern_id, pattern);
        
        vertex
    }
}
