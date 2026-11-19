---
tags: `#plan` `#context-read` `#context-insert` `#context-search` `#context-trace` `#positioned-composition` `#overlap` `#frequency` `#streaming`
summary: Migrate the context stack from contiguous child-pattern rows to positioned compositions. Retained vertices store maximal child occurrences with offsets; overlaps are explicit and gaps/wrappers are never vertices. Introduce positioned locations and navigation, replace row-based overlap closure with an occurrence merge plus seam discovery, accumulate input in owned positioned covers, and only then add streaming retention with retrospective host updates.
status: 🚧 phases 0-2 done (via the preparation plan); phase 3 next
revision: 3
date: 2026-11-19
depends_on: ../../../ngrams/agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md
prerequisite: 20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md (Phase −1 legacy isolation, and Phases 0-2 below)
related: 20261116_PLAN_OVERLAP_PROVIDER_MIGRATION.md (row-based closure; superseded for the positioned backend once Phase 4 below lands)
---

# Plan: Positioned Composition Reader Migration

**Date:** 2026-11-18
**Crates:** `context-trace` (model, locations, navigation), `context-search`
(boundary and occurrence search), `context-insert` (merge, seam discovery,
publication), `context-read` (expansion, accumulation, retention), plus
consumers in `context-api`, `context-cli`, `context-http`, `context-mcp` and
the context editor.
**Scope:** reader-side data model and algorithms. Oracle semantics and the v3
fixture format are owned by the ngrams plan.

## Revision 3 (2026-11-19)

[`20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`](20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md)
is the prerequisite of this plan: its Step 1 ("Phase −1") fences the row model
off in `legacy` modules (`context_<crate>::legacy`, default-on `legacy`
feature, legacy tests opt-in via `legacy-tests`), and its Steps 2 and 4
implement Phases 0-2 below. State after the preparation plan:

- **Phase 0 ✅** (prep Step 2.4/2.5): v2/v3 fixture lanes with separate
  manifests, `v3/scenarios.json` authoritative; new v3 scenarios
  `repeat_abcabca`, `repeat_ababa`, `seam_bcd`, `late_retention__1/__2`,
  `duplicate_observation`, `substring_input`; the `subdivision`/`visualization`
  corpus is the existing `infix` scenario.
- **Phase 1 ✅** (prep Step 2.2/2.3/2.6): `context_trace::positioned`
  (`Composition`, `Occurrence`, `OccurrenceLocation`, `OccurrencePath`,
  `IntervalView`, `PositionedGraph<G>`, `PositionedGraphRef =
  Arc<RwLock<…>>`, `from_interchange`/`to_interchange`, `diff`); every v3
  fixture round-trips. **Type deviation:** `Occurrence { offset: AtomPosition,
  child: Token }` stores a `Token` (index + width) instead of a `VertexIndex`,
  so a composition knows its children's widths without graph lookups.
  `OccurrenceLocation`/`OccurrencePath` still address children by
  `VertexIndex`.
- **Phase 2 ✅** (prep Step 4): the three operations of §3 as separate APIs —
  `context_search::boundary::{boundary_occurrences, prefixes, postfixes}`,
  `PositionedGraph::occurrences_in` and `PositionedGraph::read_interval` —
  each tested on every v3 fixture against brute-force string matching.
- `context_read::read(&PositionedGraphRef, impl IntoIterator<Item = char>)
  -> Option<Token>` exists as an `unimplemented!` stub; the positioned reader
  lane (`context-read/tests/reader_behavior.rs`) is ignored until Phase 4.
- Phase 5 now records the within-input provisional-retention rule and the
  across-input monotonicity evidence (prep plan §0.5, Step 2.6).

---

## 1. Target semantics

From the oracle plan:

- Vertices are roots, atoms and frequency-selected tokens only.
- A composite vertex stores `Composition = [(offset, child)]`: its maximal
  retained proper-child occurrences. Partial overlaps are kept; occurrences
  wholly contained in another child occurrence are removed; all positions are
  covered.
- Rows, gap views and overlap groups are optional owner-local layouts. They
  have no global identity and never influence compositions.

The reader must produce the same final semantic graph as the oracle, compared
through v3 fixtures. **No reader step may create a vertex for a gap, a wrapper,
a slice, an accumulation buffer or a successful lookup.**

## 2. Current dependencies on contiguous rows

| Area | Current assumption |
|---|---|
| `context-trace` `ChildPatterns = HashMap<PatternId, Pattern>` | A decomposition is a concatenating child sequence |
| `ChildLocation { parent, pattern_id, sub_index }` | A child is addressed by row and index |
| `postfix_iter` / prefix frames | Boundaries follow first/last row children |
| `context-search` | Matches advance through sibling indices in a row |
| `context-insert` split/join, `scope_complement`, `Overlap::close` | Overlaps are closed by constructing contiguous alternatives and interning complements |
| `context-read` `RootManager` | Accumulation appends to or replaces the tail of an owned row |
| `GraphData` v2 / fixtures | Expected structure is `string[][]` |

## 3. Core concepts to introduce

- **Occurrence location:** `(parent, offset, child)`. A path is a chain of
  occurrences; offsets add when descending.
- **Interval view:** `(owner, range)`. A read/navigation request, never a token.
- **Three distinct operations** (must not be conflated):
  1. **Boundary search:** retained occurrences starting/ending at a boundary,
     with the selected occurrence path preserved as evidence.
  2. **Occurrence enumeration in an interval:** descend through intersecting
     children, including children crossing the boundary; report whole
     retained occurrences inside the interval; deduplicate by
     `(token, owner-relative offset)`.
  3. **Content reading:** emit each atom position exactly once; may cut through
     a child without creating a token.

Key property: a maximal cover loses no occurrence of a retained token. Any
occurrence not directly listed is inside some maximal proper child; descending
reaches it in a strictly smaller vertex. Multiple paths can reach it, hence
deduplication.

## 4. Phases

### Phase 0 — Fixtures and contract

- Consume v3 oracle fixtures (`format_version: 3`) in a new reader test
  support module; keep v2 fixtures frozen and provisional.
- Add v3 scenarios needed below: `abcabca`, `ababa`, the
  `subdivision`/`visualization` corpus, a seam case where a retained token
  crosses both sides of a merge (e.g. `abc`/`cde` with retained `bcd`), and a
  late-retention case (a token that becomes retained after an earlier host
  was read).
- Never derive expectations from reader output.

**Gate:** v3 fixtures load and validate; existing v2 tests unchanged.

**Status:** ✅ done by the preparation plan, Step 2 (2026-11-19).

### Phase 1 — Positioned model in `context-trace`

- Add a positioned composition vertex form beside `ChildPatterns`, with
  reciprocal parent occurrence links `(parent, offset)`.
- Add `OccurrenceLocation`, occurrence paths and interval views.
- Add constructors from v3 `GraphData` and a validator equivalent to the
  oracle's rules (coverage, containment pruning, reciprocal links, no
  duplicate content).
- Keep the row model untouched; the backends coexist during migration.

**Gate:** v3 fixtures round-trip through the positioned graph.

**Status:** ✅ done by the preparation plan, Step 2 (2026-11-19). Deviation:
`Occurrence.child` is a `Token`, not a `VertexIndex` (see Revision 3).

### Phase 2 — Read-only navigation

- Implement the three operations from §3 over the positioned model, in
  `context-trace` (traversal) and `context-search` (boundary/prefix/postfix
  search returning occurrence paths).
- Tests on oracle fixtures only:
  - reconstruct every vertex's content;
  - boundary searches return the oracle-retained prefixes/postfixes;
  - enumeration in an interval cutting through a child finds the smaller
    retained occurrences inside it;
  - the same occurrence reached through two paths is reported once.

**Gate:** all navigation tests pass on every v3 fixture.

**Status:** ✅ done by the preparation plan, Step 4 (2026-11-19):
`context-search/src/boundary/` (boundary search with `OccurrencePath`
evidence), `context-trace/src/positioned/enumerate.rs`
(`occurrences_in`) and `read.rs` (`read_interval`). The tests compare every
vertex, every position and every interval of every v3 fixture with
brute-force string matching over the fixture's retained set, and pass
without the `legacy` feature.

### Phase 3 — Positioned merge and seam discovery in `context-insert`

Replace row closure for the positioned backend:

1. Align the accepted match with the committed suffix using its offset.
2. Project retained occurrences of both sides into the new owner's
   coordinates (whole occurrence if the side is a retained proper child,
   otherwise its cover).
3. **Seam discovery:** find retained occurrences crossing the seam and not
   wholly inside either side.
4. Deduplicate and prune by containment; keep partial overlaps.

No complement interning, no wrapper creation, no nested closure. Preserve the
accepted evidence (anchor occurrence, expansion path) as in the overlap
provider plan; only its consumer changes.

**Gate:** merging the oracle covers of overlapping observations reproduces the
oracle composition of the combined vertex, including the seam case.

### Phase 4 — Fixed-retention reader prototype

- Behind a backend switch, run `context_read::read` with the positioned model.
- The retained identity set for each test corpus is supplied (from the
  fixture), but not their offsets: the reader must discover occurrences,
  accumulate, merge and publish covers itself.
- `RootManager` keeps its ownership and commit boundary, but the in-progress
  root becomes an owned positioned cover with a committed width. Append
  extends the width and reconciles affected occurrences; an accepted overlap
  extends the same coordinate space. The real root vertex is published or
  reused only at the end of the input.
- First milestone: read `abcabca` into one owned cover and publish
  `abcabca = abca@0, abca@3` without allocating `abc`, `bca`, `bc` or any
  wrapper vertex.

**Gate:** complete-graph equality with v3 fixtures for all scenarios under
supplied retention.

### Phase 5 — Streaming retention and retrospective updates

**Retention is append-only.** The reader induces a grammar for all input
observed so far. New input may add vertices but never removes a published
token or changes the atom sequence it denotes. Compositions and reciprocal
links may change as new tokens are discovered. Every historical input must
remain exactly reconstructible after those updates.

1. **Keep lossless input observations.** Preserve each input's full atom
   sequence, either explicitly or through references to composite tokens that
   expand to that sequence. Preserve input boundaries and multiplicity: two
   observations of the same text can share a root vertex but are still two
   observations. Occurrence evidence refers to input identity and atom offset,
   not to graph edges or row references.
2. **Discover previously untokenized spans.** New input can supply additional
   occurrences in different contexts, causing a span to qualify as a token.
   Discovery must reach inside existing composites and across their boundaries
   within an input; matching only existing vertices is not enough. Keep
   candidate evidence separate from vertex storage until the retention
   criterion is met.
3. **Reconcile current and historical hosts.** On promotion, find the token's
   occurrences, including in earlier inputs and their retained hosts. For
   example, if `vi` qualifies later, revisit published `vis`. Insert each
   occurrence where visible; an eclipsed occurrence leaves that host's direct
   cover unchanged. A change to a retained child's internal cover does not
   change the whole span it exposes to its parents, but other occurrences of
   the promoted token can still affect those parents directly.

**Within-input provisional retention.** Retention is *not* monotone while an
input is still being read. Over a growing prefix of one input the batch oracle
gains and loses frequency-derived tokens (prep plan §0.5: 278 violations in the
probe; `abcab` retains `ab`, `abcabc` does not; `ababa` → `ab` lost at
`ababab`). Rule: frequency-derived tokens discovered inside an input stay
provisional (candidate evidence, not published vertices) until the input
completes; only completed-input boundaries are publication points. Atoms and
the input's own root are not affected.

**Across-input monotonicity evidence (empirical, not a proof).** Across
completed inputs no retained token was lost in any probe:

- prep plan §0.5 probe: 51 manifest scenarios plus 57 600 checks over
  3-text sequences of words of width 1-4 over 2-3 letters;
- permanent tests in `ngrams/src/tests/retention.rs`:
  `retention_is_monotone_across_inputs_of_reader_scenarios` (every v3 reader
  scenario, default), `retention_is_monotone_across_inputs_bounded_sweep`
  (196 two-text corpora over `{a,b}`, width ≤ 3, default),
  `retention_is_monotone_across_inputs_sweep` (`#[ignore]`, release:
  55 521 transitions = 54 000 from 27 000 three-text corpora over `{a,b}`,
  width ≤ 4, plus 1 521 two-text corpora over `{a,b,c}`, width ≤ 3), and the
  counter-example
  `retention_is_not_monotone_within_an_input` (`abcab` → `abcabc` loses `ab`).

Phase 5 design starts from these two facts; the proof (or a larger sweep plus
measurements of affected hosts) below is still required before cutover.

**Still open:** which index or cached evidence is needed beyond lossless input
content, and what bounds apply to retrospective discovery and affected hosts.
Local cover updates do not imply a bound on the number of affected hosts.
Prove that the batch oracle's retained set is append-only for the same
observation model, or resolve any divergence with the oracle plan before
cutover. Lossless reconstruction alone does not establish that equivalence.

**Gate:** after each completed input, every previously published token still
exists with unchanged content, and every observed input reconstructs exactly.
Compare complete graphs with independent v3 fixtures for successive corpora
without supplied retention, including late retention, duplicate observations,
and input permutations where the oracle is order-independent.

### Phase 6 — Switch default and retire row machinery

- Make the positioned backend the default for `context_read::read`.
- Migrate `context-api` compare, `context-cli` oracle tests, HTTP/MCP
  serialization and the context editor kernel to the positioned model or to an
  explicitly derived row layout.
- Optional: provide `layout(owner) -> rows` for consumers that still need
  brickwork; it must not create vertices.
- Remove row-based closure (`Overlap::close`, complement interning in
  `scope_complement`, row tail replacement) once no consumer depends on it.
  Mark the overlap provider plan superseded.
- Regenerate fixtures only from the oracle; remove v2 fixtures.

**Gate:** workspace build
(`cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app`
in the Linux codespace) and all tests pass; READMEs updated.

## 5. Risks

| Risk | Mitigation |
|---|---|
| Oracle not yet order-independent or v3 not available | Phases 0-3 of the ngrams plan are prerequisites |
| Greedy content reading used where structural search is required | Keep the three operations as separate APIs with separate tests |
| Seam occurrences missed by merging side covers | Explicit seam discovery step and dedicated fixture |
| Batch frequency disagrees with append-only retention | Phase 4 isolates frequency by supplying retention; Phase 5 requires resolving any divergence before cutover |
| Retrospective updates are unbounded | Measure affected hosts; answer the Phase 5 open questions before committing to a data structure |
| Large blast radius of the `context-trace` model change | Coexisting backends until Phase 6; no in-place rewrite of `ChildPatterns` |

## 6. Non-goals

- Changing oracle semantics (owned by the ngrams plan).
- Storing gaps, wrappers or local layouts as vertices.
- Persistence format optimizations beyond what v3 requires.
