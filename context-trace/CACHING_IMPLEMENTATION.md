# String Representation Caching Implementation

## Overview

`VertexData` now caches computed string representations to avoid repeatedly traversing the graph structure when displaying tokens in test builds. This optimization significantly improves performance when the same token is displayed multiple times.

## Architecture

### Cache Storage

```rust
#[cfg(any(test, feature = "test-api"))]
pub(crate) cached_string: std::sync::RwLock<Option<String>>,
```

- **Location**: `VertexData` struct in `context-trace/src/graph/vertex/data.rs`
- **Type**: `RwLock<Option<String>>` for thread-safe interior mutability
- **Conditional**: Only present in test builds or when `test-api` feature is enabled
- **Thread-Safety**: Uses `RwLock` instead of `RefCell` because `Hypergraph` is shared across threads via `Arc<RwLock<Hypergraph>>`

### Cache Population

The cache is populated in `Hypergraph::vertex_data_string()`:

```rust
pub(crate) fn vertex_data_string(&self, data: &VertexData) -> String {
    #[cfg(any(test, feature = "test-api"))]
    {
        // Check cache first
        if let Ok(cache) = data.cached_string.read() {
            if let Some(cached) = cache.as_ref() {
                return cached.clone();
            }
        }
    }
    
    // Compute string (existing logic)
    let s = if let Some(atom) = self.get_atom_by_key(&data.key) {
        atom.to_string()
    } else {
        assert!(data.width() > 1);
        self.pattern_string(data.expect_any_child_pattern().1)
    };
    
    #[cfg(any(test, feature = "test-api"))]
    {
        // Populate cache
        if let Ok(mut cache) = data.cached_string.write() {
            *cache = Some(s.clone());
        }
    }
    
    s
}
```

**Flow**:
1. Check if cache contains a value (read lock)
2. If cache hit, return cached string
3. If cache miss, compute string via atom lookup or pattern traversal
4. Populate cache with computed string (write lock)
5. Return computed string

### Cache Invalidation

The cache is invalidated whenever vertex children are modified:

```rust
#[cfg(any(test, feature = "test-api"))]
fn invalidate_string_cache(&self) {
    if let Ok(mut cache) = self.cached_string.write() {
        *cache = None;
    }
}

pub(crate) fn add_pattern_no_update(&mut self, id: PatternId, pat: Pattern) {
    self.children.insert(id, pat.into_pattern());
    #[cfg(any(test, feature = "test-api"))]
    self.invalidate_string_cache();
    self.validate();
}

pub(crate) fn add_patterns_no_update(&mut self, pats: Vec<(PatternId, Pattern)>) {
    self.children.extend(pats.into_iter().map(|(id, p)| (id, p.into_pattern())));
    #[cfg(any(test, feature = "test-api"))]
    self.invalidate_string_cache();
    self.validate();
}
```

**Invalidation Points**:
- `add_pattern_no_update()` - After inserting a single pattern
- `add_patterns_no_update()` - After inserting multiple patterns

This ensures the cache stays consistent when the vertex structure changes.

## Custom Trait Implementations

Since `RwLock` doesn't implement `PartialEq`, `Eq`, or `Clone`, `VertexData` uses custom implementations for test builds:

```rust
#[cfg(any(test, feature = "test-api"))]
impl PartialEq for VertexData {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width
            && self.index == other.index
            && self.key == other.key
            && self.parents == other.parents
            && self.children == other.children
        // cached_string is not compared
    }
}

#[cfg(any(test, feature = "test-api"))]
impl Clone for VertexData {
    fn clone(&self) -> Self {
        Self {
            width: self.width,
            index: self.index,
            key: self.key.clone(),
            parents: self.parents.clone(),
            children: self.children.clone(),
            cached_string: std::sync::RwLock::new(None), // Don't clone cache
        }
    }
}
```

**Key Design Decisions**:
- Cache is **not** compared in `PartialEq` (semantic equivalence ignores cache state)
- Cache is **not** cloned in `Clone` (each clone starts with empty cache)
- In non-test builds, `VertexData` uses derived implementations (no overhead)

## Performance Characteristics

### Cache Hit
- **Cost**: `RwLock::read()` + `Option::as_ref()` + `String::clone()`
- **Benefit**: Avoids graph traversal and string allocation for pattern strings

### Cache Miss
- **Cost**: Same as before + `RwLock::write()` + cache population
- **Overhead**: Minimal (one extra string clone and lock acquisition)

### Cache Invalidation
- **Cost**: `RwLock::write()` + setting `None`
- **Frequency**: Only when vertex children are modified (rare in tests)

## Usage

The caching is completely transparent to users. It automatically activates in:
- Test builds (`#[cfg(test)]`)
- Library builds with `test-api` feature enabled

No code changes required to benefit from caching. Simply use `Token::Display` or `Token::Debug` as normal:

```rust
let token = env.a;
println!("{}", token);  // First call: computes and caches
println!("{}", token);  // Subsequent calls: cache hit
```

## Thread Safety

The implementation is fully thread-safe:
- `RwLock` allows multiple concurrent readers
- Write lock required for cache updates
- Lock poisoning handled gracefully (cache miss on lock failure)
- `VertexData` remains `Send + Sync`

## Zero-Cost for Production

In production builds (without `test-api` feature):
- No `cached_string` field exists
- No cache-related code is compiled
- Zero memory overhead
- Zero runtime overhead
