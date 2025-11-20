//! Demo test to showcase the improved logging format with span indentation

use crate::init_test_tracing;
use tracing::{
    debug,
    info,
    trace,
    warn,
};

#[tracing::instrument]
fn outer_function(value: i32) -> i32 {
    info!("Starting outer function");
    trace!("Input value: {}", value);

    let result = middle_function(value * 2);

    info!("Outer function complete");
    result
}

#[tracing::instrument]
fn middle_function(value: i32) -> i32 {
    debug!("Processing in middle function");

    let result1 = inner_function(value + 10);
    let result2 = inner_function(value + 20);

    debug!("Middle function complete, combining results");
    result1 + result2
}

#[tracing::instrument]
fn inner_function(value: i32) -> i32 {
    trace!("Inner function called");
    debug!("Computing result for value: {}", value);

    if value > 50 {
        warn!("Value is quite large: {}", value);
    }

    value * 2
}

#[test]
fn test_tracing_demo() {
    let _tracing = init_test_tracing!();

    info!("=== Tracing Demo Test ===");
    info!("This test demonstrates the improved logging format");

    let result = outer_function(5);

    info!("Final result: {}", result);
    info!("=== Test Complete ===");

    assert_eq!(result, 100);
}
