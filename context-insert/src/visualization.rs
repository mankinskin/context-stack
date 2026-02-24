//! Visualization event emission for insert operations.
//!
//! Provides a thread-local step counter and helper functions for emitting
//! `GraphOpEvent` messages during split and join operations.

use context_trace::graph::visualization::{
    GraphOpEvent,
    LocationInfo,
    OperationType,
    QueryInfo,
    Transition,
};
use std::cell::Cell;

thread_local! {
    /// Step counter for insert operations.
    /// Reset when a new insert begins.
    static INSERT_STEP: Cell<usize> = const { Cell::new(0) };
}

/// Reset the step counter (call at start of each insert operation).
pub(crate) fn reset_step_counter() {
    INSERT_STEP.with(|c| c.set(0));
}

/// Get and increment the step counter.
fn next_step() -> usize {
    INSERT_STEP.with(|c| {
        let step = c.get();
        c.set(step + 1);
        step
    })
}

/// Emit a graph operation event for insert operations.
pub(crate) fn emit_insert_event(
    transition: Transition,
    description: impl Into<String>,
    location: LocationInfo,
    query: QueryInfo,
) {
    let step = next_step();
    let event = GraphOpEvent {
        step,
        op_type: OperationType::Insert,
        transition,
        location,
        query,
        description: description.into(),
    };
    event.emit();
}

/// Emit a simple insert event with just node location.
pub(crate) fn emit_insert_node(
    transition: Transition,
    description: impl Into<String>,
    node: usize,
) {
    emit_insert_event(
        transition,
        description,
        LocationInfo::selected(node),
        QueryInfo::default(),
    );
}
