//! Test that Env1 tokens show string representations

use crate::tests::env::{
    Env1,
    TestEnv,
};

#[test]
fn test_env1_tokens_have_string_repr() {
    let env = Env1::get_expected();

    // Check if token 'a' shows its string representation
    let a_display = format!("{}", env.a);
    println!("Token a: {}", a_display);

    // Check if token 'b' shows its string representation
    let b_display = format!("{}", env.b);
    println!("Token b: {}", b_display);

    // Check if token 'bc' shows its string representation
    let bc_display = format!("{}", env.bc);
    println!("Token bc: {}", bc_display);

    // These should show string representations since we registered the graph
    assert!(
        a_display.contains("\"a\""),
        "Expected token a to show string repr, got: {}",
        a_display
    );
    assert!(
        b_display.contains("\"b\""),
        "Expected token b to show string repr, got: {}",
        b_display
    );
    assert!(
        bc_display.contains("\"bc\""),
        "Expected token bc to show string repr, got: {}",
        bc_display
    );
}
