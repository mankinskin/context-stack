# Tracing and Logging Guide

This guide explains how to use the tracing infrastructure in the `context-search` crate for debugging and analyzing test execution.

## Overview

The crate uses the `tracing` library to provide structured, hierarchical logging. The test infrastructure automatically:
- Outputs formatted logs to the terminal
- Creates per-test log files in `target/test-logs/` (when enabled)
- Deletes log files when tests pass (optional)
- Preserves log files when tests fail
- Supports configurable log levels via `RUST_LOG`

## Quick Start

### Basic Usage

Add this to the beginning of your test:

```rust
use crate::init_test_tracing;
use tracing::{debug, info, warn, error};

#[test]
fn my_test() {
    let _tracing = init_test_tracing!();
    
    info!("Starting test");
    debug!(value = 42, "Processing value");
    // ... test code ...
}
```

The `_tracing` guard handles:
- Setting up structured terminal output
- Creating log file at `target/test-logs/my_test.log` (if file logging enabled)
- Formatting with timestamps, thread IDs, source locations
- Preserving logs on failure

### Custom Configuration

For more control, use `TracingConfig`:

```rust
use crate::tests::tracing_setup::{TestTracing, TracingConfig};
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;

#[test]
fn my_test_with_config() {
    let config = TracingConfig::default()
        .with_level(Level::TRACE)  // More verbose
        .stdout(false)  // Disable terminal output
        .file(true)     // Enable file output
        .span_events(FmtSpan::FULL);  // Log all span events
    
    let _tracing = TestTracing::init_with_config("my_test_with_config", config);
    
    // ... test code ...
}
```

## Current Implementation Note

**File logging** is currently limited due to the single-subscriber architecture of tracing. The system primarily logs to stdout. File logging support may be enhanced in future versions using tracing-appender or other multi-writer solutions.

For now, the best approach is to redirect test output:
```bash
cargo test my_test -- --nocapture 2>&1 | tee test_output.log
```

## Logging Levels

From most to least verbose:

- **TRACE**: Very detailed, fine-grained information
- **DEBUG**: Detailed information for debugging
- **INFO**: General informational messages
- **WARN**: Warning messages for potentially problematic situations
- **ERROR**: Error messages for failures

```rust
use tracing::{trace, debug, info, warn, error};

trace!("Very detailed information");
debug!("Debugging details");
info!("General information");
warn!("Something unexpected");
error!("Something failed");
```

## Structured Logging

Add structured fields to log messages:

```rust
use tracing::{info, debug};

// Simple field
info!(count = 42, "Processing items");

// Multiple fields
debug!(
    user_id = user.id,
    username = %user.name,  // % for Display formatting
    data = ?user.data,      // ? for Debug formatting
    "User action performed"
);
```

### Pretty-Printed Structures

For better formatting of complex data structures with proper indentation, use the `pretty()` helper:

```rust
use crate::tests::format_utils::pretty;
use tracing::debug;

let tokens = vec![token_a, token_b, token_c];

// Without pretty() - single line, hard to read:
debug!(?tokens, "Processing tokens");
// Output: tokens: [Token { index: 0, width: TokenWidth(1) }, Token { index: 1, width: TokenWidth(1) }]

// With pretty() - multi-line with indentation:
debug!(tokens = %pretty(&tokens), "Processing tokens");
// Output:
// tokens: [
//     Token {
//         index: 0,
//         width: TokenWidth(
//             1,
//         ),
//     },
//     Token {
//         index: 1,
//         width: TokenWidth(
//             1,
//         ),
//     },
// ]
```

This uses Rust's alternate Debug formatter (`{:#?}`) which provides:
- Multi-line output
- Proper indentation for nested structures
- Clear visualization of arrays and collections
- Better readability for complex data types

**When to use:**
- `?field` - For simple values or when single-line output is fine
- `field = %pretty(&value)` - For complex structures, nested data, arrays, or when you need multi-line formatted output

## Spans for Hierarchical Logging

Create spans to group related log messages:

```rust
use tracing::{info, info_span};

#[test]
fn test_with_spans() {
    let _tracing = init_test_tracing!();
    
    let span = info_span!("graph_construction");
    let _enter = span.enter();
    
    info!("Building graph");
    // All logs here are inside the span
    
    drop(_enter); // Exit span
    
    info!("Outside the span");
}
```

Or use the `#[tracing::instrument]` attribute:

```rust
use tracing::instrument;

#[instrument]
fn process_data(value: i32) {
    debug!("Processing");
    // Function entry/exit automatically logged
}
```

## Filtering

### Environment Variable

Set `RUST_LOG` to control what gets logged:

```bash
# Only INFO and above
RUST_LOG=info cargo test

# Trace for specific module
RUST_LOG=context_search::search=trace cargo test

# Multiple modules with different levels
RUST_LOG=context_search=debug,context_trace=info cargo test

# Everything from context_search
RUST_LOG=context_search cargo test
```

### Programmatic Filtering

Use `TracingConfig::with_filter()`:

```rust
let config = TracingConfig::default()
    .with_filter("context_search::search=trace,context_trace::graph=debug");
let _tracing = TestTracing::init_with_config("test_name", config);
```

## Configuration Options

### `TracingConfig` Builder Methods

```rust
TracingConfig::default()
    .with_level(Level::TRACE)           // Set default level
    .with_filter("module::path=level")  // Custom filter directives
    .stdout(true/false)                 // Enable/disable terminal output
    .file(true/false)                   // Enable/disable file output
    .log_dir("custom/path")             // Change log directory
    .span_events(FmtSpan::FULL)         // Configure span logging
```

### Span Event Configuration

Control what span events are logged:

```rust
use tracing_subscriber::fmt::format::FmtSpan;

TracingConfig::default()
    .span_events(FmtSpan::NEW | FmtSpan::CLOSE)  // Log span creation and closure
    .span_events(FmtSpan::ENTER | FmtSpan::EXIT) // Log every span enter/exit
    .span_events(FmtSpan::FULL)                  // Log everything
    .span_events(FmtSpan::NONE)                  // Don't log span events
```

## Log File Management

### Automatic Cleanup

By default, log files are deleted when tests pass:

```rust
#[test]
fn passing_test() {
    let _tracing = init_test_tracing!();
    // If this test passes, target/test-logs/passing_test.log will be deleted
}
```

### Preserving Logs

Keep logs even for passing tests:

```rust
#[test]
fn test_keep_logs() {
    let _tracing = init_test_tracing!().keep_log();
    // Log will be preserved regardless of test result
}
```

### Failed Tests

When tests fail, logs are automatically preserved:

```bash
❌ Test failed! Log file preserved at: target/test-logs/failing_test.log
```

## Example: Debugging a Failing Test

```rust
use crate::{init_test_tracing, search::Find};
use tracing::{info, debug, warn, instrument};

#[instrument]
fn build_test_graph() -> Hypergraph<BaseGraphKind> {
    debug!("Creating new hypergraph");
    let mut graph = Hypergraph::default();
    
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    debug!(?a, ?b, "Inserted atoms");
    
    let ab = graph.insert_pattern([a, b]);
    info!(?ab, "Created pattern");
    
    graph
}

#[test]
fn test_search_with_tracing() {
    // Enable TRACE level for detailed debugging
    let config = TracingConfig::default()
        .with_level(Level::TRACE)
        .with_filter("context_search=trace");
    let _tracing = TestTracing::init_with_config("test_search_with_tracing", config);
    
    info!("Starting search test");
    
    let graph = build_test_graph();
    let graph_ref = HypergraphRef::from(graph);
    
    let query = ['a', 'b'];
    debug!(?query, "Performing search");
    
    let result = graph_ref.find_sequence(&query[..]);
    
    match &result {
        Ok(response) => {
            info!(?response, "Search succeeded");
            if response.is_complete() {
                debug!("Got complete match");
            } else {
                warn!("Got partial match");
            }
        }
        Err(e) => {
            error!(?e, "Search failed");
        }
    }
    
    assert!(result.is_ok());
}
```

If this test fails, check `target/test-logs/test_search_with_tracing.log` for detailed trace information.

## Advanced: Instrumenting Library Code

Add tracing to the library code for better visibility:

```rust
use tracing::{debug, instrument, trace};

#[instrument(skip(graph))]
pub fn find_sequence<'a, G, A>(
    graph: &'a HypergraphRef<'a, G>,
    query: &[A],
) -> SearchResult
where
    G: GraphKind,
    A: AsAtom<G::Atom>,
{
    trace!(query_len = query.len(), "Starting sequence search");
    
    // ... implementation ...
    
    debug!(?result, "Search completed");
    result
}
```

## Tips and Best Practices

1. **Start with INFO level**: Use INFO for high-level test flow, DEBUG for details
2. **Use structured fields**: Add context with `field = value` syntax
3. **Group with spans**: Use spans to organize related operations
4. **Filter strategically**: Use `RUST_LOG` to focus on specific modules
5. **Instrument key functions**: Add `#[instrument]` to important functions
6. **Check log files on failure**: Always review preserved logs when debugging

## Disabling Tracing

To run tests without any tracing overhead:

```rust
// Simply don't call init_test_tracing!()
#[test]
fn fast_test() {
    // No tracing setup
    // ... test code ...
}
```

Or disable specific outputs:

```rust
let config = TracingConfig::default()
    .stdout(false)
    .file(false);
// Tracing is initialized but output is suppressed
```

## Summary

- **Quick start**: Add `let _tracing = init_test_tracing!();` to your test
- **Logs location**: `target/test-logs/<test_name>.log`
- **Auto cleanup**: Logs deleted on success, preserved on failure
- **Filtering**: Use `RUST_LOG` env var or `TracingConfig::with_filter()`
- **Levels**: TRACE → DEBUG → INFO → WARN → ERROR
- **Structured**: Add fields with `info!(field = value, "message")`
- **Hierarchical**: Use spans to group related logs
