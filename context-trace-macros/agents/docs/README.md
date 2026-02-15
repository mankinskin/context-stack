# context-trace-macros

Procedural macro crate providing instrumentation macros for the context-engine.

## Overview

This crate provides procedural macros that enhance tracing by automatically capturing function signatures and type information, making trace output more informative.

## Macros

### `instrument_sig`

An attribute macro that wraps `#[tracing::instrument]` and automatically adds:

- **Function signature** (`fn_sig` field): The complete function signature as a string
- **Self type** (`self_type` field): For methods, the concrete type of `Self`
- **Associated types**: Any associated types used in the function

#### Usage

```rust
use context_trace_macros::instrument_sig;

#[instrument_sig]
fn my_function(arg1: i32, arg2: &str) -> Result<(), Error> {
    // ...
}
```

This expands to include `fn_sig = "my_function(arg1: i32, arg2: &str) -> Result<(), Error>"` in the span fields.

### `instrument_trait_impl`

Applies `instrument_sig` to all methods in a trait implementation block.

#### Usage

```rust
use context_trace_macros::instrument_trait_impl;

#[instrument_trait_impl]
impl MyTrait for MyType {
    fn method1(&self) { }
    fn method2(&mut self, x: i32) { }
}
```

## Benefits

- **Debugging**: Function signatures in traces help identify exactly which function overload is being called
- **Type visibility**: Self type information helps track generic implementations
- **Automatic**: No manual string maintenance required
