---
tags: `#plan` `#context-read` `#context-insert` `#overlap` `#closure` `#structure`
summary: Preserve the authoritative shared-token occurrence, accepted search response, boundary paths and insertion evidence through expansion materialization and nested closure. Implement one path-driven provider in context-insert, reuse scoped split/join machinery, and carry translated structural connections across publication instead of repeatedly reconstructing them from widths.
status: 🧊 frozen for legacy (phases 1-3 and 6 implemented; phases 4-5 will not be done on the row model)
revision: 3
date: 2026-11-16
supersedes: revision 2 of this plan and the overlap-bundling sections of 20260315_PLAN_COMPLEMENT_AND_C3.md
blocking: none for staged migration; affected-host completion and historical-pattern retention require the explicit correctness phases below
---

# Plan: Structure-Preserving Overlap Closure

**Date:** 2026-11-16 (revision 3)
**Primary crate:** `context-insert`
**Consumers:** `context-read`; workspace consumers of the insertion API.
**Scope:** preserve structural evidence from postfix traversal and expansion
search through materialization, nested closure, publication and root integration.
Retire the legacy overlap implementation after the new provider is active.

This is no longer just a relocation of the current closure code. First preserve
behavior while repairing the dataflow; then address the missing affected-host
completion and historical-pattern retention under separate, explicit gates.
Verified n-gram oracle fixtures should be authoritative; hand-written graph-shape
assertions alone do not justify a semantic requirement. Current snapshots are
provisional pending oracle correctness, as noted below.

## Legacy freeze note (2026-11-19)

The row reader moved to `context_read::legacy` (and insert/search/trace to their
`legacy` modules) per `20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`
Step 1. `tests/reader_behavior.rs` is now `tests/legacy_reader_behavior.rs`.
Its 6 known failures are fixed and marked
`#[ignore = "legacy known failure: final graph differs from v2 oracle"]`:
`complex_overlap`, `infix`, `multiple_overlaps`, `nested_complements`,
`flat_complement`, `accepted_materialization`. The list is checked by review,
not by a test.

**Frozen for legacy; only fixes to keep the legacy lane green.** No further
phases of this plan are implemented on the row model; phases 4-5
(affected-host completion, historical-pattern retention) are taken over by the
positioned reader plan (`20261118_PLAN_POSITIONED_COMPOSITION_READER.md`,
Phases 3-5: occurrence merge with seam discovery instead of row closure). The
6 known failures above stay ignored and are not fixed here. Since the
test-suite cutover (prep plan Step 2.7) the whole legacy lane is
`ignore = "legacy suite"` by default; run it with
`cargo test -p context-read --features legacy-tests` whenever legacy code is
touched. The plan is retired (superseded) when the reader plan's Phase 6
removes row closure.

## Test contract correction (2026-11-17)

The test refactor supersedes references below to preserving existing synthetic
expectations unchanged. Public reader behavior is now checked against generated
final corpus snapshots in `context-read/tests/fixtures`, through
`context-read/tests/reader_behavior.rs`. Internal cursor, closure-path,
scheduling, and sink contracts remain separate in `src/tests/cursor.rs` and use
oracle-backed graph seeds. Normal reader tests never run the oracle.

Comparisons reject missing **and extra** tokens/decompositions, duplicate
content, and inconsistent edge metadata. The former occurrence-completeness
checker was removed; its stronger normalization claims must be justified
separately. The experimental oracle changes were reverted; the original
order-dependent algorithm remains. Current snapshots were regenerated with
that unchanged oracle and record one observed final output per corpus. The
`ngrams` CLI now generates one top-level `vertices` object per corpus (format
version 2); cumulative-prefix and oracle-seeded behavioral tests were removed.
The ngrams hash-order regression exercises both root iteration orders for the
same corpus and semantic labels. Resolve oracle correctness before treating
these snapshots as authoritative expectations.

The original regressions are covered by the `complex_overlap` and
`multiple_overlaps` behavioral scenarios. Other newly exposed mismatches must
also be resolved against generated expectations, not by accepting the reader's
current graph. Historical suite counts and commands below describe the earlier
migration checkpoint; see `context-read/README.md` for current validation and
fixture regeneration commands.

## Revision 3: corrections to the previous plan

1. **Keep and use paths.** Withdraw revision 2's proposal to replace boundary
   paths with `find_represented` searches. The fact that current consumers
   discard path contents is a design defect, not evidence that paths are useless.
2. **Treat `P` as authoritative.** Postfix traversal selects its occurrence and
   provides the anchor path. Closure must not rediscover it by offset and width.
3. **Materialize the accepted search result.** Do not discard the probe response
   and search the query a second time.
4. **Preserve connections across graph rewrites.** A pre-insertion path/cache is
   not automatically valid after publication. Translate requested occurrences
   through the existing plan and edits, rather than recovering them by DFS.
5. **Unwind paths for nested closure.** Reuse sibling ranges, resolved splits,
   interval references and closure results. Do not rerun postfix traversal or
   atom-expand complements at every level.
6. **Use scoped split/join as the common mechanism.** The legacy provider's
   path-driven organization is worth keeping; its direct sibling insertion and
   invalid bundling formulas are not.
7. **Track affected containing occurrences.** The failing tests demonstrate
   obligations beyond the two immediate overlap paths. These are an explicit
   correctness phase, not something path preservation alone is claimed to fix.

## Implementation status (2026-11-16)

Phases 1-3 and the retirement in Phase 6 are implemented; graph shapes are
unchanged. Suites: trace 118 (+6 doc), search 57, insert 54 (9 legacy
bundle tests removed, 11 added), read 101 passed with the two known failures
and byte-identical diffs.

- **Phase 1:** `InsertCtx::materialize(response)` /
  `ToInsertCtx::materialize` materialize an accepted response;
  `insert_next_match` wraps search + materialize. `expand_root` materializes
  its probe response; the duplicate query search is gone.
- **Phase 2:** `publish` returns a `Publication` (composed original-range
  replacements and created vertices, as ordered stages).
  `Publication::translate` re-indexes boundary paths and enters replacement
  tokens through their boundary decompositions (descending only through
  vertices created by that publication), failing explicitly otherwise.
  `expand_root` translates its traversal frames with
  `Publication::translate_chain`, which also follows wider boundary children
  of patterns the publication rewrote or added (recorded per stage), so
  postfixes created by materialization join the chain as the former
  re-traversal did; the anchor `boundary_path` DFS is removed.
  `Materialization::prefix_path` projects `E -> P` from the accepted search:
  the start occurrence of a reused whole-root match
  (`Response::entire_root_start_path`, new in context-search) or, for a
  created `E`, first children of created vertices down to an existing one,
  then the trace-cache edges below it translated through the publication.
  Frames the search simplified away all lie at offset 0 and are completed
  positionally (`overlap::prefix_frames`).
- **Phase 3:** `insert::scoped::scope_complement` reads complements from frame
  siblings (single sibling used directly), else uses the sibling width with
  the existing narrowing/wrapper/reconciliation core, seeded from the frame's
  decomposition, and returns its `Publication`. `context_insert::Overlap`
  with `ClosurePolicy::{Nested, SingleStep}` is the only closure executor: `T`
  once, steps fixed to the supplied frame parents and unwound inner to outer
  (wrappers created by complement resolution lengthen the frames between
  steps but are not steps), remaining frames translated after every
  resolution, existing unions found through known parent
  decompositions, then one structured `[Si,T]` lookup (no atom expansion).
  `postfix_chain`, `complement`, `atoms`, `boundary_path` and
  `validate_path` were removed from read.
- **Phase 6:** `bundle_overlap`, `OverlapBundleInput`, `PartitionOutcome` and
  the `ToInsertCtx::bundle_overlap` forwarding were removed with their tests;
  docs were updated.

Decisions and open items:

- Split maps and interval references are not shared across closure frames:
  each frame's complement is resolved after the previous publication, and
  per §3.3 translated results (frames, `T`, `C(i-1)`) are reused rather than
  stale split caches.
- Multi-sibling complements use the siblings' width only inside the shared
  reconciliation core (alternative decompositions, narrowing, wrappers), as
  §4.1 permits; single siblings are used directly.
- Closure decompositions `[Si,T]` and `[Li,C(i-1)]` are still added directly
  (with structured reuse of an existing union). Join cannot plan a new
  union vertex or reuse `C(i-1)` graph-wide (it recognizes existing tokens
  only as single children of split nodes), and reconciling `Si`'s other
  decompositions (e.g. `deabc=[dea,bc]`) changes graph shape. Deferred to
  Phase 4 by decision.
- Phases 4 (affected-host completion) and 5 (historical-pattern retention)
  are not started; `complex_abcabababcaba` and `read_multiple_overlaps1` fail
  with their original diffs.

---

## 1. Meaning of overlap and closure

```text
anchor A:       L P
expansion E:      P T
closure C:      L P T = [A, T] = [L, E]
```

- `P` is a proper postfix selected by traversal of the accumulated anchor.
- `E` is the accepted created or reused expansion token, extending from that
  same `P` into the remaining context. It is the overlapping token.
- The anchor-side postfix path and expansion-side prefix path identify the
  **same token at the relevant occurrences**, not merely equal-width intervals.
- Closure integrates `E` with the relevant anchor scope and returns its union
  token. Root ownership and installation remain the sink's responsibility.
- The triple `(anchor, shared, expansion)` describes the overlap relationship;
  it is not itself a behavioral executor. Keep the existing `Overlap` vocabulary
  if useful, but do not add another synonym such as `OverlapSeam` just to rename
  the triple. Its actual input must include the structural connections.
- Use **complement**, not "partition", for `L` and `T`. `Partition<R>` already
  names a different split/join concept in `context-insert`.

### 1.1 Selection policy is not changed by this migration

Today `expand_root` visits proper postfixes largest-first and accepts the first
whose confirmed match consumes new input. The match for that selected postfix
is authoritative; closure does not select another expansion.

This is not a comparison of all postfix candidates to maximize consumed input.
Do not silently change that policy under the phrase "largest left-most".
Preserve the block-seam bound and read-only rejected probes.

### 1.2 Closure bound and sink integration

Read chooses a structural bound `X` on the retained anchor path:

- Editable root: `X` is its entire last child; commit replaces that tail.
- Otherwise: `X = A`; commit adopts the whole-root closure.

An editable root is sink-owned, single-pattern and parentless. Recheck this
after expansion materialization, which can add decompositions or parents.
Choose the bound and the corresponding sink operation together.

If `X == P`, the result is already `E`. No complement insertion or closure
allocation is needed. This is a narrowed scope, not an improper overlap or an
empty complement. A root-level proper overlap still has nonempty `L` and `T`.

Self-expansion (`E == A`) currently disables tail editing and closes only at
the anchor level. Preserve that policy initially, but run it through the same
executor with selected frames, not a second closure implementation.

### 1.3 Nested closure follows retained anchor frames

For anchor frames `P = S0 < S1 < ... < Sk = X`, unwind from `P` outward:

```text
C0 = E
Si = Li + S(i-1)
Ci = [Si, T] = [Li, C(i-1)]
```

`Li` comes from siblings before the selected postfix child of the frame.
Resolve `T` by unwinding the expansion prefix path and its right siblings.
These are structural ranges; widths are derived metadata, not a replacement
for their identity. Some unary witness frames may have no siblings; they do
not constitute a new proper closure step or require an empty-complement token.

Example:

```text
anchor:     abc = [a, bc], bc = [b, c]
expansion:  cd = [c, d]
paths:      abc -> bc -> c, cd -> c

T   = d
bcd = [bc, d]  = [b, cd]
abcd = [abc, d] = [a, bcd]
```

One-step closure would instead be `[abc,d] = [ab,cd]`, not `[a,cd]`.
The nested result retains the intermediate union `bcd`.

---

## 2. Source and log evidence

The investigation preceding this revision read the core code and reran:

```sh
cargo test -p context-read --lib -- complex_abcabababcaba read_multiple_overlaps1 closure_closes_nested_suffix_before_outer_scope --test-threads=1
```

Result: **one passed, two failed**. The nested regression passes; the two
original failures remain. Logs are generated, not committed:

- `target/test-logs/complex_abcabababcaba.log`
- `target/test-logs/read_multiple_overlaps1.log`

Reproduce the baseline when starting implementation; do not assume generated
logs or earlier suite counts remain current.

### 2.1 Structural information currently discarded

| Implementation | Current behavior | Required change |
|---|---|---|
| `context-read/src/expansion/mod.rs::expand_root` | Keeps only confirmed width from `find_ancestor`, then calls `insert_next_match` to search the same query again. | Pass the accepted `Response` to materialization. |
| Same function | Collects the traversal path, then replaces it with a new DFS witness after insertion. | Preserve its occurrence and nesting frames through publication. |
| `context-insert/src/insert/outcome.rs` | Carries `Response`, but read takes only `.token()`. | Pass the structural materialization result onward. |
| `context-insert/src/insert/result.rs` | `IndexWithPath.path` is extracted from the query cursor. | Do not mistake this query path for a graph path from `E` to `P`. |
| `context-insert/src/insert/scoped.rs` | Path adapters validate a path, turn it into a width, and delegate to width-driven discovery. | Seed/continue scoped processing from the supplied structural boundary. |
| `context-read/src/expansion/closure.rs::postfix_chain` | Traverses again after the accepted traversal. | Use retained frames and their translated connections. |
| `close_step` and `complement` | Rediscover represented intervals, reconstruct and discard paths, atom-expand pieces for global lookups. | Reuse scoped resolutions, planned intervals and known parent occurrences. |
| `context-insert/src/join/plan.rs::publish` | Resolves candidates and applies edits, but returns only the target token. | Resolve the narrowly requested structural continuation evidence too. |

### 2.2 Existing facilities to reuse

- `context-trace/src/trace/traceable/{mod,role,root}.rs`: path-to-trace edges.
  Verify direction, ordering and coordinates for the supplied boundary role;
  do not assume every current helper already implements the required adapter.
- `context-insert/src/interval/init.rs`: `InitInterval` consumes a root, cache
  and end bound.
- `context-insert/src/split/vertex/mod.rs`: split completion reconciles a
  known boundary with other decompositions.
- `context-insert/src/split/{context,provenance}.rs`: records why boundaries
  were requested and the parent-decomposition incidences that propagated them.
  Provenance is currently informational, not an existing completion engine.
- `context-insert/src/join/context/{frontier,node/merge/node}.rs`: bottom-up
  merging shares resolved splits and host intervals with enclosing nodes.
- `context-insert/src/join/plan.rs`: candidate references, original-range
  replacements and overlays avoid stale indices during planning.
- `context-insert/src/join/select.rs`: decides retained structure before
  publication. Preserve its rules during dataflow migration.

### 2.3 What the failing logs establish

**Complex example:** an owned root grows to `abcabab`, insertion groups its
tail as `abab`, and tail replacement grows it to `abcababab`. Later extraction
of `abcaba` changes that host's decompositions. Final nested suffix closure
does produce `ababcaba`, `abababcaba` and the correct final content, but:

```text
missing tokens: abcabab, abcababa
actual abcababab:   [abc,ababab] and [abcaba,b,ab]
expected:          [abc,ababab] and [abcababa,b]
extra historical pattern: ababab = [abab,ab]
```

**Multiple overlaps:** extraction of `deab` from `cdeab` and closure with `abc`
produce `deabc`, but existing `bcdea` lacks `[bc,dea]`. It retains only
`[bcde,a]` and `[b,cdea]`.

`bcdea` is not a frame of the current anchor `deab`. Two retained boundary
paths alone cannot discover every required host. Conversely, these failures
do not justify scanning the whole graph or propagating every split to all
parents indiscriminately.

---

## 3. Target dataflow and evidence lifetime

```text
postfix traversal
  -> authoritative shared occurrence + ordered anchor frames
accepted search Response
  -> materialization of that exact response
  -> expansion token + translated boundary connections + affected evidence
path-driven scoped complement resolution
  -> reused splits / interval references / resolved complements
bottom-up closure
  -> closure token + outstanding affected-host obligations
scoped completion and publication
  -> prepared sink operation
sink commit
  -> root receipt -> next iteration
```

Use the smallest operation-local carrier needed for this flow, not a public
snapshot of every search/split/join cache. Prefer existing rooted role paths,
`ChildLocation`, `JoinRef`, split maps and incidence records over parallel
representations. Exact Rust signatures follow the implementation's types;
the contracts below are mandatory.

### 3.1 Accepted expansion carrier

Carry:

- Anchor and the authoritative shared token, with its traversal occurrence.
- Ordered original anchor frames, including the selected child/sibling ranges.
- Confirmed match extent and accepted search response until materialization.
- The resulting `E` and a valid prefix connection to the same `P`.
- Translated continuation frames or plan references sufficient for closure.
- Only the affected incidence/host evidence needed for later completion.

Keep original semantic frames distinguishable from their post-edit witnesses:
an arbitrary shorter witness must not silently erase an intermediate closure
obligation. Frames newly introduced by accepted materialization must also be
accounted for when they belong to the selected nesting chain. Derive these
from the edits/overlay rather than restarting postfix traversal.

### 3.2 Consume the accepted response without another search

Extract the response-handling body of `insert_next_match_impl` into a shared
materialization entry point. The current searchable API searches once and
delegates to it; expansion probing passes its already accepted response.

Preserve `Complete`, `NoExpansion` and `Created` semantics, error reporting,
confirmed match length and the existing checkpoint/candidate distinction.
Do not replace checkpoint validation with the candidate cursor position.
Do not install an expansion whose extent differs from the accepted match.

For reused whole-root matches, derive the `E -> P` connection from the accepted
search trace/start occurrence; a no-insertion outcome still needs that evidence.

### 3.3 Publication must translate requested occurrences

Start with the existing staged publication model. Extend its internal result
only enough to resolve requested continuation references through:

1. Candidate selection, including inlining or dropping helper candidates.
2. Candidate-to-token materialization.
3. Composed original-range replacements and added patterns.

The result must identify the resulting occurrence, not merely its content.
An inlined witness must translate through the retained parent representation;
silently retaining a stale child index is invalid. If required evidence cannot
be resolved, surface an explicit invariant failure, not a fallback DFS.

Do not reuse the original search `TraceCache` wholesale: its root may be the
larger host from which `E` was extracted and its locations may predate edits.
Rebase/project only the required structural evidence. Reuse caches within
their validity period; after publication reuse translated results, not stale
split maps.

A combined expansion/closure plan is a possible later optimization, not a
prerequisite. Do not redesign all insertion transactions just to avoid a small
continuation result. No rollback or cross-operation cache lifetime is promised.

---

## 4. One structural closure provider

Put graph-only overlap execution in `context-insert::overlap`. Read selects
the bound/policy and wraps the result as a sink operation. Keep module layout
minimal; split files only where the actual responsibilities warrant it.

### 4.1 Path-driven scoped boundary resolution

Make the path-based scoped adapters genuine structural entry points:

1. Validate supplied parent-child connectivity and boundary role directly.
   `boundary_target` can perform this without searching.
2. Walk retained frames/sibling ranges to seed trace-cache boundaries.
3. Reconcile alternative decompositions through existing split completion.
4. Merge complements bottom-up using the shared split and interval maps.
5. Return the resolved token/reference together with any continuation evidence
   needed by the enclosing closure frame.

Use the same split/join core for prefix and postfix complements; only geometry
and trace direction differ. Preserve wrapper-boundary handling and scope
narrowing behavior covered by existing scoped tests.

Widths/offsets remain legitimate internal coordinates for matching a boundary
across alternative decompositions and tracking original ranges through edits.
Compute them from retained structural frames and reuse them. They must not be
the public substitute for a known occurrence path.

Keep width-only APIs for callers that really have only a range. Such adapters
may discover structure once at entry and then delegate to the common core.
Do not make the path-based API delegate back to width-based rediscovery.

`find_represented` may remain for range-only callers, assertions and tests.
It is not the authoritative locator for `P` or the nested closure iterator.

### 4.2 Closure execution and canonical reuse

- Unwind the expansion prefix connection to resolve `T` once; reuse its result.
- Unwind the selected anchor frames from inner to outer, resolving `Li` and
  proposing `Ci = [Si,T] = [Li,C(i-1)]`.
- Pass resolved references/results to the next frame, rather than finding
  paths back into the just-created token.
- Feed complement construction and pattern reconciliation through existing
  scoped split/join planning and publication, not direct sibling insertion.
- Reuse operation-local candidates, known decompositions and host occurrences
  before considering a content lookup. Verify how existing canonical tokens
  are found; `JoinPlan`'s pattern-key deduplication alone is not proof of
  graph-wide content uniqueness.
- If existing machinery still needs a canonical lookup, use the structured
  token pattern once at its ownership boundary and cache the result for this
  operation. Do not atom-expand/search the same complement at each frame.
  Any unavoidable conversion must have a documented reason and focused test.
- Preserve explicit errors for disconnected canonical content during the
  behavior-preserving phases; do not replace them with duplicate allocation.
- Empty-bound and self-expansion policy select work for this same executor.
  Empty-bound returns `E` without resolving an unused `T`.

### 4.3 Affected containing occurrences

Carry obligations from actual edited/participating occurrences. Use recorded
incidences and, where necessary, scoped reverse parent links to reach relevant
containing hosts such as `bcdea`; the search trace alone need not contain them.

Before enabling completion, define and test:

- Which host occurrences are affected by each inserted complement/closure.
- How a boundary translates through the specific parent decomposition.
- When a host is inside the permitted completion scope.
- How `(host, occurrence/range, required boundary)` obligations are deduplicated.
- A finite worklist over structurally justified obligations; repeated
  occurrences of the same token must remain distinct when their positions differ.

Reuse the same boundary resolution and merging functions for these hosts.
This is not another closure formula or an unrestricted upward graph traversal.
The exact relevance rule must be established with minimal fixtures before
claiming the two failing tests are fixed.

### 4.4 Root update remains separate

`RootManager::commit` performs no overlap search:

- `ReplaceTail`: replace the expected last child of the eligible owned root,
  updating width and child-parent metadata while keeping root identity.
- `ReplaceRoot`: adopt the completed union and relinquish ownership.
- Return `CommitReceipt`; expansion records it before starting the next step.

Keep this distinction: it is ownership policy, not scoped versus non-scoped
closure. Scoped means restricting structural work to the selected occurrences;
it does not imply a second closure algorithm.

---

## 5. Migration phases and validation gates

Each phase must compile independently and preserve all previously passing
tests. Use focused tests first, then the affected crate suites. Do not hard-code
final suite counts: new tests and retirement of obsolete legacy tests change
them. The earlier full-suite reference was trace 118, insert 52, read 101/2.

### Phase 1: retain accepted search evidence

1. Extract shared response materialization in `insert/context.rs`; keep the
   searchable API as a wrapper.
2. Have `expand_root` retain the accepted response and original traversal frames.
3. Remove the duplicate query search without losing outcome distinctions.
4. Keep current reconnection temporarily only until Phase 2 replaces it;
   do not first implement revision 2's path deletion.

Tests: existing-token and split-created expansions, rejected probes remaining
read-only, and checkpoint/candidate extent. Check retained response/trace identity
or a focused existing search-event hook, not timing as a proxy for no re-search.
Run cursor tests and the two known failing cases; no graph-shape changes yet.

### Phase 2: structural continuation through insertion

1. Preserve requested boundary/frame references through plan/select/publish.
2. Resolve valid post-publication connections for anchor and expansion.
3. Handle complete/reused results as well as inserted subranges of larger hosts.
4. Remove accepted-expansion reconnection DFS only once these paths are supplied
   by the materializer.

Tests: insertion rewrites the anchor; helper inlining changes witness depth;
two occurrences share a token but not a position; `E` is extracted from a
larger host; no-insertion reuse and self-expansion. Assert actual locations
connect to the intended `P` and required nesting frames are not lost.

### Phase 3: structural scoped complements and provider migration

1. Replace the path-to-width adapters with path-to-trace/scoped resolution,
   reusing existing split completion, wrapper handling and shared merge maps.
2. Add the graph-only path-driven closure provider and switch read to it in
   the same bounded migration. Avoid keeping a second active closure executor.
3. Replace `postfix_chain`, per-step `boundary_path`, repeated represented-range
   discovery and repeated atom lookups with retained/translated frames and
   resolved references.
4. Keep direct tail replacement allocation-free and self-expansion behavior
   unchanged. Keep sink scope selection/integration in read.

Tests: existing nested regression; nested expansion prefix complements;
alternative decompositions crossing a boundary; wrapper conflicts; canonical
result reuse; successive tail closures; direct tail; self-expansion.
Preserve exact oracle-verified graph-shape expectations. Inspect the active call chain
to verify known paths do not feed a width-based discovery loop.

Gate after Phases 1-3: no new failures; the two known failures may remain.
If either becomes green, retain the original expectation and explain why.

### Phase 4: affected-host completion

1. Add minimal fixtures for missing containing-host decompositions, including
   the `bcdea` relationship, and for the complex example's missing prefixes.
2. Define the bounded incidence/parent relevance rule from those fixtures.
3. Process retained obligations through the shared scoped resolver and merger.
4. Assert unrelated hosts stay unchanged; include repeated-occurrence and
   alternative-decomposition cases.

This phase intentionally changes graph structure to satisfy existing tests.
Re-run both original failures and inspect their exact remaining diffs. Do not
declare the complex test fixed while its extra historical pattern remains.

### Phase 5: historical-pattern retention, only where still required

The complex oracle rejects `ababab=[abab,ab]`, which already existed before
later structure was discovered. Moving closure into a provider does not remove
that pattern, and current selection must not be assumed to prune old patterns.

If still failing, define a narrowly justified retention/replacement rule using
the new structural evidence, with tests preserving legitimate decompositions.
Use the existing planning/selection/publication boundary; do not special-case
the example's token content. Gate: both original scenarios match generated oracle fixtures and no
previously passing read/insert/trace test regresses.

### Phase 6: retire legacy surface and update documentation

1. Remove legacy `bundle_overlap`, `OverlapBundleInput`, `PartitionOutcome`,
   sibling-collection helpers, caller-supplied `self_overlap`, exports and
   `ToInsertCtx` forwarding methods after a workspace consumer check.
2. Replace width-only legacy bundle tests with tests of real shared occurrences
   and exact graph structure. Keep `tests/env/expanded_overlap.rs` while its
   separate insertion tests use it.
3. Run affected crate suites separately so a failure in one cannot conceal
   another crate's result. Run `cargo build --workspace --all-targets` to check
   downstream API consumers.
4. Update:
   - `context-insert/src/overlap/mod.rs` module documentation.
   - `context-insert/agents/docs/overlap/index.yaml` and the parent docs index.
   - `context-read/docs/{architecture,notation,overlap-refactor-tasks}.md`.
   - `context-read/README.md` and this plans index.
   - A superseding note on bundling sections of
     `20260315_PLAN_COMPLEMENT_AND_C3.md`, preserving its historical content.

Document evidence lifetime, path translation, allowed internal offset use,
bounded affected-host completion and sink ownership separately.

---

## 6. Risks and explicit limits

| Risk | Required mitigation |
|---|---|
| Retained locations become stale after insertion. | Translate requested occurrences through selection and composed edits; validate returned connectivity directly. No silent DFS fallback. |
| A fresh witness skips required nested tokens. | Retain semantic frames separately from rewritten witnesses; test intermediate closure structure. |
| Search cache describes a larger host, not `E`. | Project/rebase the specific occurrence through materialization instead of relabeling the cache root. |
| Selector inlines a referenced helper. | Translate through the selected representation; test this before removing reconnection. |
| Only one decomposition is processed. | Retain the authoritative witness and complete its boundary across alternative decompositions through split machinery. |
| Structural dedup misses an existing canonical token. | Verify existing reuse guarantees; permit a narrowly bounded structured lookup rather than speculative duplicate insertion. |
| Completion mutates unrelated parents or loops. | Explicit relevance/scope rule, occurrence-aware deduplication and finite worklist tests. |
| Self-expansion changes accidentally. | Preserve the one-step policy through the shared executor and test repeated content. |
| Refactor is presented as fixing the oracle. | Separate behavior-preserving gates from completion and historical-retention gates. |
| API migration expands excessively. | Keep evidence operation-local, preserve compatibility wrappers where useful, and build downstream crates. |

Non-goals: changing expansion selection priority, unrestricted all-parent
normalization, globally persistent caches, a generic transaction framework,
generalizing scoped insertion to arbitrary infixes, or unrelated search/API
cleanup. Do not remove useful width-only APIs or `find_represented` solely to
make a symbol count smaller.

---

## 7. Acceptance criteria

- [ ] Postfix traversal is the authoritative source of `P` and its occurrence.
- [ ] The accepted response is materialized without searching that query again.
- [ ] Anchor and expansion structural evidence reaches closure; query paths are
      not confused with graph occurrence paths.
- [ ] Publication supplies translated continuation evidence, including helper
      inlining and composed replacements; no stale location is reused.
- [ ] Nested closure unwinds retained frames and reuses resolved complements,
      splits and interval/closure results instead of rebuilding the chain.
- [ ] Path-based scoped entry points actually consume paths structurally;
      known witnesses do not round-trip through width-only discovery.
- [ ] Offset calculations remain inside structural boundary reconciliation,
      not as a substitute for already known paths.
- [ ] Exactly one graph-only overlap closure executor is active in
      `context-insert`; read retains only selection/scope policy and integration.
- [ ] Empty-bound tail replacement does no unused complement work, and
      self-expansion initially preserves the existing policy.
- [ ] Affected hosts are completed through justified scoped occurrences, with
      explicit deduplication/termination and unrelated-host preservation.
- [ ] Both original failing scenarios match generated oracle fixtures after the correctness phases,
      including the complex oracle's historical-pattern requirement.
- [ ] No prior passing tests regress; downstream workspace targets build.
- [ ] Legacy bundle APIs are retired and directly related docs are updated.
