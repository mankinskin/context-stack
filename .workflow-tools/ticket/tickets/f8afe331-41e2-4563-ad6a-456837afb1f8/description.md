## Bug: Regression panic in `integration::dedup_tests::dedup_atoms_not_duplicated`

### Status
**Regression** — this test was previously passing. It now panics.

### Reproduction

```sh
cargo test -p context-cli --test cli_integration -- dedup_atoms_not_duplicated --nocapture
```

### Panic Output

```
thread 'integration::dedup_tests::dedup_atoms_not_duplicated' panicked at
crates/context-stack/context-trace/src/graph/vertex/data/children.rs:80:13:
Pattern vertex has no children VertexData {
    token: T0w1,
    key: VertexKey(8800fea7-1841-4f45-ae7e-cd108e868b5b),
    parents: {
        2: Parent {
            width: TokenWidth(2),
            pattern_indices: { PatternIndex { pattern_id: PatternId(3e254569-...), sub_index: 0 } },
        },
    },
    children: {},
}
```

### Test Code (tools/cli/context-cli/tests/integration/dedup_tests.rs)

```rust
fn dedup_atoms_not_duplicated() {
    let mut ws = TestWorkspace::new("dedup-atoms");
    ws.add_atom('a');
    ws.add_atom('b');
    ws.insert_text("ab"); // should reuse existing atoms
    let atoms_result = ws.list_atoms();
    let atoms = unwrap_atom_list(&atoms_result);
}
```

### Root Cause (Preliminary)

A vertex `T0w1` (atom 'a', width 1) has a parent entry recording width=2 (indicating it participates in a compound pattern), but `children: {}` is empty. When `children.rs:80` attempts to iterate the vertex's children to reconstruct the pattern, it asserts the vertex must have children and panics.

This is likely caused by `insert_text("ab")` when atoms 'a' and 'b' were pre-created — the insert path creates a compound token linking the two atoms but does not write the child edges back into both atom vertices. The atom vertex for 'a' acquires a parent reference to the compound token but no corresponding child entry.

Relevant file: `crates/context-stack/context-trace/src/graph/vertex/data/children.rs:80`

### Fix Direction

Investigate `insert_sequence` / `insert_next_match` in `context-insert` to ensure that when a compound token is created from pre-existing atoms, both child edges (atom → compound) and parent edges (compound → atom) are written symmetrically. The vertex data for each atom must have its `children` field populated to match the parent references.

### Acceptance Criteria

- `cargo test -p context-cli --test cli_integration -- dedup_atoms_not_duplicated` passes without panic.
- No other dedup tests regress.
- `cargo test -p context-cli --test cli_integration` shows at most the pre-existing failures.