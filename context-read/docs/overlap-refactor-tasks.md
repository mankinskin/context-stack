# Root-anchor overlap refactor

Implementation task list and verification status for the agreed design. See [notation](notation.md)
for `I`, `b`, `c`, `e`, `R` and the commit seam `I[b,c)`.

## Design summary

- **One anchor.** The accumulated root `R` is the only overlap anchor. There is
  no seam probing, no separate local anchor and no `pending` span.
- **One step per iteration.** Each step consumes at least one atom:
  1. **Root expansion:** follow the postfix chain of `R`; for each postfix
     token `P` with `|P| <= c - b`, largest first, search `P + I[c,e)`.
     Accept the first expansion `E` with `|E| > |P|`, then close and commit.
  2. **Forward step:** otherwise, append the largest match at `c`. Its
     overlaps are found by root expansion in the next step.
- **One closure.** Every overlap is closed by the same path-based operation
  and committed immediately.
- **Sink applies, never searches.** `RootManager` applies `Append`,
  `ReplaceTail` or `ReplaceRoot`, validating ownership and staleness.

### Established rules

1. **Postfix chain.** In a valid graph, the proper postfix tokens of `t` form a
   chain: each next element is the widest last child among the current token's
   patterns. Traversal follows only that child; postfixes wider than the seam
   are traversed but not searched.
2. **Graph invariant.** `context-read` relies on unique tokens, represented
   occurrences and distinct boundaries across decompositions. Violations from
   other crates are bugs tracked separately, not handled by read logic.
3. **Closure scope `X`.** For an editable root, `X` is the root's last child
   `X1`. For a non-editable root, `X` is the root.
4. **Whole-child overlap (`P == X1`).** If `E` is not the root vertex, replace
   `X1` by `E` without creating a token. If `E` is the root vertex, freeze the
   root and close at root scope.
5. **Closure.** With `X = L + P` and `E = P + T`, create
   `C = [X, T...] = [L..., E]`. Take `L` from the unique decomposition of `X`
   with a boundary at `|L|`, if present; otherwise use left siblings on the
   path to `P`. Derive `T` the same way within `E` at `|P|`. Never allocate a
   token solely for a complement.
6. **Search mutation.** Probe candidates read-only. Materialize only the
   accepted `E`, then recompute root editability from the graph.
7. **Failure handling.** Compute the closure plan read-only before applying it.
   Invariant violations panic; there is no rollback.

### Expected results

| Input/state | Result |
| --- | --- |
| `aaa`, `E` is root `aa` | `aaa = [aa,a] = [a,aa]` |
| `abcabcabc`, `E` is root `abcabc` | `[abcabc,abc] = [abc,abcabc]` |
| `aabbaabb`, `b` does not expand | `aabb = [a,a,b,b]`, root `[aabb,aabb]` |
| editable `[x,y,z,abc]`, `E = bcd` | `abcd = [abc,d] = [a,bcd]`, root `[x,y,z,abcd]` |
| then `E = cde` | `abcde = [abcd,e] = [a,b,cde]`, root `[x,y,z,abcde]` |

## Tasks

### 0. Worktree cleanup

- [x] Restore `context-trace/src/lib.rs` to `HEAD`'s existing
  `PostfixIterator` export.
- [x] Delete the untracked `context-trace/src/tests/postfix_iter.rs`.
- [x] Run `cargo check -p context-trace -p context-read` (passes with existing warnings).

### 1. Postfix chain in `context-trace`

- [x] Fix the existing `postfix_iter` implementation to traverse the
  invariant-based postfix chain, widest first. Reuse the existing iterator
  and public entry point; do not add a parallel iterator type.
- [x] Select the widest last child among the current token's patterns at each
  step; stop at an atom.
- [x] Test the alternative-decomposition graph
  `abcde = [a,bcde] = [abc,de]`, `bcde = [b,cde] = [bc,de]`; expect
  `bcde, cde, de, e`.
- [x] Preserve connected paths for closure by accumulating the yielded
  `ChildLocation` steps. Adjust the existing API only if necessary.
- [x] Preserve prefix-iterator behavior where implementation machinery is
  shared; validate directly affected callers.

### 2. Graph validation helpers

- [x] Add a test-only validator for unique token content, represented
  occurrences reachable within each token and distinct top-level boundaries
  across decompositions.
- [x] Use it in new `context-read` overlap tests.
- [x] Record the `abcd`/`cdab` insertion violation as a separate
  `context-insert` task.

### 3. Root expansion in `ExpansionCtx`

- [x] Remove `probe_seam`, `seam_dirty`, `first_candidate`, `seam_root`,
  `pending`, `find_overlap` and `build_overlap_state`.
- [x] Store the latest committed root from `CommitReceipt`.
- [x] Implement `expand_root`: traverse the postfix chain, skip `|P| > c - b`,
  probe `P + I[c,e)` read-only, accept the first crossing expansion and return
  `P`, its path and the probe result.
- [x] Materialize only the accepted `E`; reject results extending beyond the
  block.
- [x] Make `next()` perform either one root-expansion step or one forward step,
  and advance the cursor by the newly consumed atoms.
- [x] Skip root expansion for an empty seam (`c == b`) or when no root exists.
- [x] Keep `Response::checkpoint_position()` for read-only probe lengths.
- [x] Reconnect accepted postfix paths after materialization rewrites patterns.
- [x] Reuse existing combined roots in forward steps without sink-side search.

### 4. Shared closure module

- [x] Replace `collapse_seam`, `materialize`, `atoms_of`, `BandState`,
  `OverlapChain`, `Band`, `OverlapLink` and `BandCapLink` with one closure
  module.
- [x] Recompute editability after materializing `E`; select `X` by rules 3–4.
- [x] Derive a prefix path from `E` to `P`; panic if either path is missing.
- [x] Compute `L` and `T` by rule 5, reusing an existing boundary decomposition
  when present.
- [x] Return a prepared closure plan before mutating the graph.
- [x] Apply the plan: replace `X1` by `E`, create `[X,T...]` and `[L...,E]`, or
  close at root scope.
- [x] Reuse an existing token with the closure content when one exists.
- [x] Remove `left_partition_from_postfix_path` and
  `right_partition_from_prefix_path` from `context-read` call paths.

### 5. Root integration

- [x] Replace `Span` with `Append(Token)`,
  `ReplaceTail { previous, old, token }` and
  `ReplaceRoot { previous, token }`.
- [x] Return `CommitReceipt { root }`; expansion recomputes editability from
  the graph.
- [x] Validate `ReplaceTail`: current root, ownership, one pattern, no parents,
  `old` as last child, width increase and no self-reference.
- [x] Add a `context-trace` primitive for width-changing owned-tail replacement
  that updates widths, parent links and parent pattern metadata.
- [x] Mark `ReplaceRoot` results as adopted, non-editable roots.

### 6. Tests

- [x] Keep `repetition_aaa_both_decompositions`, `repetition_abcabcabc`,
  `repetition_aabbaabb`, `repetition_abcabc` and `repetition_abcxyzabc`
  passing with exact patterns.
- [x] Add `xyzabcd` and `xyzabcde` tests with exact patterns, vertex counts and
  validator checks.
- [x] Test scope selection for `X1 = wabc = [w,abc]`, preserving `wabc`.
- [x] Test canonical complements with existing `wa` and `X = [w,abc] = [wa,bc]`;
  expect `L = [wa]`.
- [x] Test `P == X1` replacement without a new token, and freezing when `E` is
  the root vertex.
- [x] Test that probing does not mutate the graph and that an `E` created inside
  the root makes it non-editable.
- [x] Test sink rejection of stale, non-editable and self-referential tail
  replacements.
- [x] Remove `expansion_rejects_unrepresented_seam_complement` and replace
  `expansion_closes_seam_across_multiple_root_children` with closure-module
  tests.
- [x] Update `validate_mixed_pattern` to expect flat `aabb = [a,a,b,b]`.
- [x] Run focused tests, `cargo test -p context-trace`,
  `cargo test -p context-read --lib` and
  `cargo test -p context-insert overlap_bundle`.

### 7. Documentation

- [x] Rewrite [architecture](architecture.md) for root expansion, closure rules
  and the three span operations.
- [x] Add `X`, `L`, `P`, `E`, `T`, `X1` and the postfix chain to
  [notation](notation.md); remove seam-position scanning.
- [x] Add the `xyzabcd`, `xyzabcde` and `wabc` cases to
  [examples](examples.md).
- [x] Replace seam-probing references in [graph changes](graph-changes.md) with
  the postfix-chain argument and root validity reasoning.
- [x] Update `context-read/README.md`'s commit protocol description.

## Verification and separate follow-ups

The root-anchor implementation is in place. Passing focused cases do not prove
that existing insertion output always satisfies its required invariants.

- `cargo test -p context-trace`: 114 unit tests and 6 doc tests pass.
- `cargo test -p context-insert overlap_bundle`: 9 tests pass.
- All nine repetition regressions pass, including exact `aabbaabb` structure.
- `cargo check -p context-trace -p context-read` passes.
- `cargo test -p context-read --lib`: 97 pass; four previously failing
  test cases remain. Their
  assertions were not weakened:
  - `complex_abcabababcaba` reaches an insertion partition panic.
  - `read_infix1` produces a different retained decomposition.
  - `read_infix2` receives prefix coverage where it expects an entire token.
  - `read_multiple_overlaps1` lacks an expected additional decomposition.

Separate tasks, outside this refactor:

- [ ] **context-insert occurrence completeness:** insertion of `ab` into
  initially flat `abcd`/`cdab` must represent both occurrences, not rewrite
  only one token.
- [ ] **context-insert partition handling:** investigate the
  `join/partition/info/pattern_info.rs` panic reached by the complex repetition.
- [ ] **Existing read/search expectations:** investigate the three remaining
  legacy read failures without weakening canonical occurrence/boundary checks.
