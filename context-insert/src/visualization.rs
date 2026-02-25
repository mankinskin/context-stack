//! Visualization event emission for insert operations.
//!
//! Provides a thread-local step counter and helper functions for emitting
//! `GraphOpEvent` messages during split and join operations.

use context_trace::graph::{
    search_path::{PathTransition, VizPathGraph},
    visualization::{
        GraphOpEvent,
        LocationInfo,
        OperationType,
        QueryInfo,
        Transition,
    },
};
use std::cell::Cell;
use std::cell::RefCell;

thread_local! {
    /// Step counter for insert operations.
    /// Reset when a new insert begins.
    static INSERT_STEP: Cell<usize> = const { Cell::new(0) };

    /// Path identifier for the current insert operation.
    static INSERT_PATH_ID: RefCell<String> = RefCell::new(String::new());

    /// Accumulated path graph for the current insert operation.
    static INSERT_VIZ_PATH: RefCell<VizPathGraph> = RefCell::new(VizPathGraph::new());
}

/// Reset the step counter, generate a new path_id, and reset the path graph
/// (call at start of each insert operation).
pub(crate) fn reset_step_counter() {
    INSERT_STEP.with(|c| c.set(0));
    let id = format!(
        "insert-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    INSERT_PATH_ID.with(|c| *c.borrow_mut() = id);
    INSERT_VIZ_PATH.with(|c| *c.borrow_mut() = VizPathGraph::new());
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
    path_transition: PathTransition,
    description: impl Into<String>,
    location: LocationInfo,
    query: QueryInfo,
) {
    let step = next_step();
    let path_id = INSERT_PATH_ID.with(|c| c.borrow().clone());
    // Apply transition to accumulated path graph
    INSERT_VIZ_PATH.with(|c| {
        let _ = c.borrow_mut().apply(&path_transition);
    });
    let path_graph = INSERT_VIZ_PATH.with(|c| c.borrow().clone());
    let event = GraphOpEvent {
        step,
        op_type: OperationType::Insert,
        transition,
        location,
        query,
        description: description.into(),
        path_id,
        path_transition,
        path_graph,
    };
    event.emit();
}

/// Emit a simple insert event with just node location.
pub(crate) fn emit_insert_node(
    transition: Transition,
    path_transition: PathTransition,
    description: impl Into<String>,
    node: usize,
) {
    emit_insert_event(
        transition,
        path_transition,
        description,
        LocationInfo::selected(node),
        QueryInfo::default(),
    );
}
