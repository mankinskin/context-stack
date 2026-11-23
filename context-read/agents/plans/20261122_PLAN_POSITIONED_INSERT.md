---
tags: `#plan` `#context-insert` `#context-trace` `#context-search` `#positioned-composition` `#insert` `#split` `#join`
summary: Positioned insertion, revision 3 (supersedes revision 2 in 20261120_PLAN_POSITIONED_INSERT). insert_match(Response) turns a matched span into a token so that the graph equals the oracle graph of the corpus plus the span. Split positions with a start/end role are seeded from the response and pushed into every child occurrence containing them (legacy rule); required intervals are derived per vertex (target, postfix, prefix, overlap); a bottom-up join creates the new tokens through GraphEdit and returns a Publication. Defines the graph invariants, the response guarantees and why every created token is new, unique and complete. GraphEdit is the only mutation layer (atom, create, add_occurrences, extend), vertex-local and strictly checked, used only by insertion and the reader; insert_atom, insert and insert_composition are removed. Responses carry a generation stamp; no rollback; expected graphs are generated oracle fixture files (v3/insert lane). Root expansion (the legacy seam operation) belongs to the reader plan.
status: ✅ implemented (tasks 3.0–3.5, 2026-11-23) — revision 3; insert gate 1,379/1,379; deviations in §16
date: 2026-11-22
parent: 20261120_PLAN_POSITIONED_READER_OPEN_WORK.md (step 3)
supersedes: 20261120_PLAN_POSITIONED_INSERT.md (revision 2)
depends_on: 20261120_PLAN_POSITIONED_SEARCH.md (done)
consumer: 20261122_PLAN_POSITIONED_READER.md
---

# Plan: Positioned Insertion (revision 3)

**Crates:** `context-trace` (`GraphEdit`, `Publication`, generation,
validation, insert fixtures), `context-insert` (split graph, join,
`insert_match`), `context-search` (generation stamp in `Response`).
**Roadmap:** step 3 of
[20261120_PLAN_POSITIONED_READER_OPEN_WORK](20261120_PLAN_POSITIONED_READER_OPEN_WORK.md).
**Previous revision:**
[20261120_PLAN_POSITIONED_INSERT](20261120_PLAN_POSITIONED_INSERT.md)
(revision 2). Its Part A, the walkthrough of the legacy insertion, is still
accurate and is not repeated here.
**Consumer:** the positioned reader,
[20261122_PLAN_POSITIONED_READER](20261122_PLAN_POSITIONED_READER.md).
**Prototype:**
[`split_positions.py`](../../../context-insert/agents/prototypes/split_positions.py)
(execution manual in the prototypes
[README](../../../context-insert/agents/prototypes/README.md)).

## 0. Review changes (2026-11-22, 2026-11-23)

| # | Question | Resolution |
|---|---|---|
| 1 | What is the binary search on compositions, and how does it give the crossing children? | §4.2 step 2: offsets and ends of a composition both increase strictly, so the occurrences strictly containing a position form one contiguous index range, found by two `partition_point` calls (`Composition::crossing`). It is a lookup, not part of the algorithm: scanning all children gives the same set |
| 2 | The seam scan was not the legacy seam operation | Removed from both plans. The reader keeps legacy root expansion (largest postfix token first, search `[postfix, rest]`, insert the accepted match, integrate it into the root), see [reader plan](20261122_PLAN_POSITIONED_READER.md) §3.2. Insertion is unaffected: it inserts one match, whichever step found it |
| 3 | `GraphEdit` must not become a second way to build graphs | §8 rewritten: a vertex-local edit layer with three checked operations (`create`, `add_occurrences`, `extend`), callable only by insertion and the reader (feature-gated, guard test). Tables of which algorithm uses which operation |
| 4 | Reader integration | The reader inserts every accepted match with `insert_match`; response roots may be the input root, an owned parentless vertex (22 of 57 insertions in reader prototype R0). The reader holds no response or path across an edit, so no `Publication::translate` is needed (§7) |
| 5 | Decisions on P1–P4 (2026-11-23) | Confirmed as D8–D11 (§14): `GraphEdit` is the only way to change vertices and compositions, the other mutation functions are removed (§8.5); generation stamp as safeguard; no rollback, failures are prevented by the implementation and the invariants; expected graphs are generated fixture files (task 3.1.1) |

## 1. Changes from revision 2

| # | Revision 2 | Revision 3 | Why |
|---|---|---|---|
| 1 | Split graph stores a set of **intervals** per vertex; an extra closure step adds intersections; intervals are pushed into one child (`covering`/`ending_at`) or into the containing child | Split graph stores **split positions** per vertex, each with a role (start or end border). A position is pushed into **every** child occurrence that strictly contains it (legacy rule). Intervals are derived per vertex when the join needs them | Closer to legacy (`SplitVertexCache` of positions). Each vertex knows its own width, and each edge has its offset, so every interval can be computed locally. No closure step, no special case for interval pushes, no skipped children in the join. Same result on all 1,247 cases |
| 2 | Uniqueness of new tokens: reuse a token whose composition was already created in this insertion | New tokens are **provably new and pairwise distinct** (§6, J1–J2). Reuse by composition is replaced by a debug assertion | The invariants (G2, G5) imply it. The prototype never created a composition twice |
| 3 | Invariants listed informally (F1–F4, I2) | Graph invariants G1–G5, response guarantees R1–R5 and join postconditions J1–J6 (§3, §6) | The algorithm is now described in terms of what it may assume and what it must establish |
| 4 | `Cover`, cover projection, `Cover::merge` with seam scan (task 3.5) | **Removed.** Tokens crossing a junction of the read input are found by the reader's root expansion (legacy `expand_root`), reader plan §3.2 | Insertion never needs a seam: the host contains the whole span. The reader keeps the legacy seam operation, which needs no cover merge |
| 5 | `GraphEdit { create, recompose, finish }`, `Publication::translate` | `GraphEdit { atom, create, add_occurrences, extend, finish }` (§8), the only mutation layer, shared by insertion and the reader. The `Publication` is the insert result. **No translation** | The join needs `create` and `add_occurrences`; the reader's owned root needs `extend`. No caller holds paths across an insertion (reader plan §4.3) |
| 6 | Navigation counter for intersection pushes | Counter of split positions that are not on a response path edge | Measures how much of the split graph the response provides (75%) |

## 2. Goal and contract

```rust
pub fn insert_match<G: GraphKind>(
    graph: &PositionedGraphRef<G>,
    response: Response,
) -> Result<InsertOutcome, InsertError>;
```

**Contract.** Let `C` be the corpus of an oracle-consistent graph, and let
`response` be a search result on that graph with root `R` and span `[o, e)`.
Write `N = R[o, e)`; content is used only to state the contract. After
`insert_match`:

- the graph equals `oracle(C + [N])`, i.e. the graph as if `N` had been read
  as one more input;
- the returned token is `N`'s vertex.

`EntireRoot` responses change nothing (`N = R` is a token). Insertion makes
exactly one match a token. Which matches are inserted is decided by the
reader: as in legacy, every match accepted by a forward step or a root
expansion that is not a token (reader plan §3, §5). The response root may
be the reader's input root, an owned parentless vertex; it is handled like
any other root.

The three steps (as legacy `materialize` = split + join + publish):

1. **Split:** build the split graph from the response. Seed the root with
   the span borders, then push every split position into every child
   occurrence that contains it.
2. **Join:** bottom-up by width, create every required new token, smallest
   first, and finally the target token `N`.
3. **Publish:** return the `Publication` (created tokens, edited vertices)
   in `InsertOutcome::Created`.

## 3. Invariants

### 3.1 Graph invariants (hold before and after every insertion)

Notation: `v[x, y)` is the substring of vertex `v`'s string at atom offsets
`x..y`; an *occurrence* `t@x` in `v` means `v[x, x + |t|) = t`. Strings
appear only in definitions and proofs; the algorithm never reads content.

| # | Invariant | Checked by |
|---|---|---|
| G1 | **Unique content.** No two vertices have the same string. | `validate()` (`DuplicateContent`, debug/test-api) |
| G2 | **Complete maximal composition.** For a composite `v`, `comp(v)` is exactly the set of maximal occurrences of all vertices that are proper substrings of `v`: every occurrence `t@x` of a vertex `t ≠ v` inside `v` lies inside some listed occurrence (or is one), and no listed occurrence lies inside another. Consequences: offsets and ends both strictly increase, compositions are gap-free (atoms are vertices) and have at least 2 occurrences. | Structure: `Composition::new`, `validate()`. Completeness: oracle diff in tests |
| G3 | **Reciprocal links.** `c@a ∈ comp(v)` ⇔ `(v, a) ∈ parents(c)`. | `validate()` |
| G4 | **Overlap closure.** If two occurrences in a composition overlap, their overlap is a vertex. | `validate()` (structural check added in task 3.1) |
| G5 | **Unique minimal host.** For a string `s` that occurs in some vertex but is not a vertex, there is exactly one *minimal host occurrence*: a pair `(v, x)` with `v[x, x + |s|) = s` and no child of `v` containing that occurrence. Every other occurrence of `s` lies inside an occurrence of `v`. | Prototype `invariants.py` on all fixtures. The formal argument against the oracle's frequency rule is open (risk) |

G2 together with G1 means that the graph is determined by its vertex set:
every composition is a function of which strings are vertices. So
"insertion is correct" reduces to "the right strings become vertices, and
the compositions that contain them are recomputed".

G5 is why a non-token string can be found and inserted at a single place.
A string with two minimal host occurrences would occur in two different
contexts. The oracle would then count it more often than each of its
superstrings and retain it.

### 3.2 Response guarantees (positioned search, validated in search plan 2.6)

| # | Guarantee |
|---|---|
| R1 | `response.root_token()` is `R` and `response.span()` is `[o, e)` in root offsets. |
| R2 | **Narrowest root:** no child occurrence of `R` contains `[o, e)`. |
| R3 | `EntireRoot` ⇔ `[o, e) = [0, |R|)`. |
| R4 | **Start path:** a connected chain of occurrences from `R` down to the largest token starting at `o`. Every step except the last has a child that strictly contains `o`. |
| R5 | **End path:** a connected chain from `R` down to the largest token ending at `e` (canonical, `RolePath::end_at`). Every step except the last has a child that strictly contains `e`. |
| R6 | (new, D9) `response.generation()` is the graph generation under which the search ran. |

### 3.3 Consequences used by the algorithm

- **C1** If the response is not `EntireRoot`, `N` is not a vertex: by G2,
  a vertex occurring in `R` lies inside a child of `R` or is `R`. Neither is
  possible, by R2 and R3.
- **C2** `(R, o)` is the unique minimal host occurrence of `N` (R2, G5).
- **C3** Every new token is `N`, a prefix of `N` or a postfix of `N`. It is
  the part of a vertex occurrence that lies inside the span. This holds for
  all 2,119 new tokens of the gate.
- **C4** Only *split nodes* (vertices that receive a split position) change.
  An existing vertex in which a new token occurs maximally is that token's
  minimal host, and the join creates it there (J5).

## 4. Split graph (successor of `InitInterval`, `SplitVertexCache`, split propagation)

### 4.1 Data

```rust
pub struct SplitGraph {
    root: Token,
    span: Range<AtomPosition>,               // [o, e)
    nodes: BTreeMap<VertexIndex, SplitVertex>, // keyed by index: Token's Ord compares width only
}
pub struct SplitVertex {
    width: TokenWidth,
    positions: BTreeMap<AtomPosition, Roles>, // relative to this vertex, 0 < p < width
}
pub struct Roles { start: bool, end: bool }   // which span border(s) the position stands for
```

A **position with role** is the coordinate of a span border inside one
occurrence of the vertex. *Start* (S) means this occurrence of the vertex
contains `o` at that position. *End* (E) means it contains `e` there. This
is the legacy `SplitPositionCache` without rows: a row-relative
`(sub_index, inner_offset)` is replaced by the occurrence offset, which the
composition stores already. The role replaces the legacy `RootMode` and
the augmentation step. It tells which side of the split lies inside the span.

### 4.2 Seed and propagation

1. **Seed:** the root gets `o: S` and `e: E`, but no position at its own
   borders: `Prefix` gives only `e: E`, `Postfix` only `o: S`. `EntireRoot`
   has no split graph.
2. **Propagate** top-down in decreasing `(width, index)`. A vertex is
   processed once all wider ones are done, so its positions are final. For
   each position `p` with role `r` of `v`, and for each occurrence `c@a` of
   `v` with `a < p < a + |c|`, add `(p − a, r)` to `c`. These occurrences are
   a contiguous range of the composition, because offsets and ends strictly
   increase; they are found by binary search (`Composition::crossing(p)`,
   new). A position on a child's border is not pushed: the border already
   lies between children.

   *Binary search.* G2 forbids containment between listed occurrences, so
   for `i < j`: `a_i < a_j` and `a_i + |c_i| < a_j + |c_j|`. The predicates
   "ends after `p`" and "starts before `p`" are therefore monotone along the
   composition, and the occurrences with `a < p < a + |c|` are the index
   range

   ```text
   lo = partition_point(|occ| occ.end() <= p)     // first occurrence ending after p
   hi = partition_point(|occ| occ.offset < p)     // one past the last starting before p
   crossing(p) = occurrences[lo..hi]
   ```

   The range has more than one element when `p` lies in the overlap of
   adjacent occurrences (`cabcab = cabc@0, bcab@2`, `p = 3`: both). This is
   how `covering` and `ending_at` already work (one `partition_point`
   each). It only replaces a scan over all children; the split rule is "every
   occurrence strictly containing `p`", however it is found.
3. **Response reuse:** every step of the response's start path except the
   last is a split edge `(parent, a, child)`. It gives the child the
   position `p_parent − a` with role S; the end path does the same with
   role E. The split run starts from these edges (asserted in debug builds)
   and adds the other crossing children from `crossing(p)`. On the gate,
   1,969 of 2,632 non-root positions (75%) are path edges.
4. **Termination and size:** widths strictly decrease along pushes. No
   vertex had more than 2 S and 2 E positions. The split phase is read-only.

Why every crossing child, not only the path child: the join composes a piece
from each child that crosses an interval border (§5.2). With the uniform
rule, that piece always exists. Pushing only along `covering`/`ending_at`
fails 208 of 1,247 cases (`split_positions.py --paths-only`). Positions that
are not on a path never bound a newly composed interval. Their tokens are
existing tokens, and as pieces they were always discarded by `maximal` (347
pieces). Skipping them is a possible optimization (§14, open), not part of the
design.

### 4.3 Required intervals (derived, not stored)

Each vertex needs the tokens for the parts of its occurrences that lie
inside the span:

| Vertex | Positions | Required intervals |
|---|---|---|
| root `R` | `o: S`, `e: E` | target `[o, e)` only |
| other split node `v` | S positions `s`, E positions `t` | postfix `[s, |v|)` per `s`; prefix `[0, t)` per `t`; **overlap** `[s, t)` per pair with `s < t` |

- A postfix `[s, |v|)` is the part of an occurrence of `v` that crosses the
  start border and lies inside the span. That occurrence overlaps the new
  token in its parent, and by G4 the overlap must become a token. The same
  holds for prefixes at the end border.
- An overlap `[s, t)` is needed because, after the join, `v`'s new postfix
  and prefix occurrences overlap inside `v`, again G4. Without overlaps, 60
  periodic cases fail (`--no-overlaps`). Overlaps of overlaps are again
  pairs, so the set is closed.
- The root needs no postfix or prefix: legacy wrappers and complements are
  not tokens.

## 5. Join (successor of the join frontier, partitions, merge, select, publish)

### 5.1 Order

Split nodes in increasing `(width, index)`, so children come before parents.
Inside a node, its required intervals come in increasing width, with ties
by start: an overlap before the postfix and prefix containing it, and the
target last. A map `(VertexIndex, start, end) → Token` holds the resolved
intervals of this insertion.

### 5.2 Resolving an interval `[x, y)` of `v`

1. **Inside a child.** If an occurrence `c@a ∈ comp(v)` contains `[x, y)`,
   the interval resolves to `c` when it is equal to `c`. Otherwise it
   resolves to the token already resolved for `(c, x − a, y − a)`. That entry
   exists: `x` is `0` or an S position, and `y` is `|v|` or an E position.
   Every S or E position strictly inside `c` was pushed into `c` with the
   same role, so `c` has the matching postfix, prefix or overlap. No vertex is
   created.
2. **Compose.** Otherwise collect the pieces, shifted to `x`:
   - each child `c@a` inside `[x, y)` → `c@(a − x)`;
   - each child crossing a border of `[x, y)` → the token resolved for
     `(c, max(x, a) − a, min(y, a + |c|) − a)`, at `max(x, a) − x`. It exists
     for the same reason as in 1;
   - `v`'s own tokens already created for intervals inside `[x, y)` (its
     overlaps).

   The composition is `maximal(pieces)`: sort by `(start, wider first)` and
   keep an occurrence only if it ends after all earlier ones. A new vertex is
   created through `GraphEdit::create`.
3. **Update the node.** After all of `v`'s intervals,
   `comp(v) := maximal(comp(v) ∪ {t@x : t composed for v's interval [x, y)})`
   through `GraphEdit::add_occurrences`. Occurrences now inside a new token
   move under it: they lose the link to `v` and stay reachable through the
   new token.

The join reads compositions from the live graph. `v`'s own composition is
unchanged until step 3, and its children are already final, so no overlay
or plan-then-apply phase is needed. The root's target resolves to `N`, which
is created last.

## 6. Correctness of the created tokens

Each statement assumes G1–G5 and R1–R5 for the graph before the insertion.
Proofs are sketched; the oracle gate checks the result.

- **J1 New content.** A composed interval `v[x, y)` is not a vertex. If it
  were a vertex, then by G2 its occurrence at `x` would lie inside a child of
  `v` (the interval is proper: non-root intervals are proper by definition,
  and the root target by R3). That is case 1 of §5.2.
- **J2 Pairwise distinct.** Two composed intervals have different strings. A
  composed interval lies in no child of its vertex, so it is a minimal host
  occurrence of its string (a non-vertex, by J1). By G5 there is only one
  such occurrence per string. So no token is created twice, and no
  content-keyed or composition-keyed lookup is needed (D3). A debug
  assertion checks this.
- **J3 Valid composition.** The pieces cover `[x, y)` without gaps, because
  `comp(v)` covers `v`. No single piece covers `[x, y)`, since otherwise
  case 1 would apply, so `maximal` keeps at least 2 occurrences.
  `Composition::new` accepts the result.
- **J4 Complete composition (G2 for new tokens).** Take an occurrence `u@z`
  of a vertex inside `v[x, y)`.
  - If `u` existed before, it lies inside a child `c` of `v` (G2 for `v`),
    so inside `c`'s piece. That piece is `c` itself, or a token whose
    composition is complete by induction over the join order.
  - If `u` is new, its occurrence in `v` is either inside a child (same
    argument) or a minimal host occurrence. By G5 it is then `u`'s own
    interval in `v`, which is a piece.
- **J5 Only split nodes change.** A new token `u` occurs maximally in an
  existing vertex `w` only if that occurrence lies in no child of `w`, i.e.
  it is `u`'s minimal host occurrence (G5). The join created `u` at that
  node and added it in step 3. Every other vertex keeps its composition.
  This holds in all cases of the gate.
- **J6 Overlap closure (G4 after).** New overlaps arise only between a new
  occurrence and the occurrences crossing its borders. These are the
  postfix, prefix and overlap intervals of §4.3, which are resolved before
  the occurrences that contain them. This is checked by `validate()` after
  every insertion in debug/test-api builds.

G1 after the insertion follows from J1 and J2. G3 holds because
`GraphEdit` maintains links. G5 after the insertion is not proved; it holds
because the result equals the oracle graph (gate).

## 7. Result: `InsertOutcome` and `Publication`

Legacy outcome names are kept (`insert/outcome.rs`):

```rust
pub enum InsertOutcome {
    /// EntireRoot and the query is exhausted: `N` is the existing root.
    Complete { token: Token, response: Response },
    /// EntireRoot with query remaining: the caller continues after `token`.
    NoExpansion { token: Token, response: Response },
    /// The span became a new token.
    Created { token: Token, response: Response, publication: Publication },
}
pub struct Publication {
    /// Created tokens in creation (join) order; the last is `N` (§16).
    pub created: Vec<Token>,
    /// One entry per changed existing vertex.
    pub edits: Vec<VertexEdit>,
}
pub struct VertexEdit {
    pub vertex: VertexIndex,
    pub added: Vec<Occurrence>,
    pub removed: Vec<Occurrence>,
}
```

`InsertError`: `StaleResponse` (generation mismatch) and
`Composition(PositionedError)` (a violated invariant, i.e. a bug; D10).

Convenience wrappers as in legacy: `insert(graph, query) -> Result<Token,
_>` and `insert_next_match(graph, query) -> Result<InsertOutcome, _>` search
and then call `insert_match`. Only the matched part is inserted; the caller
continues from the response's checkpoint. No path translation through a
`Publication` is planned: the reader keeps only tokens and offsets across
an edit (reader plan §4.3).

## 8. `GraphEdit`: the only mutation layer (D8)

### 8.1 Principle

Vertices and compositions are changed **only** through `GraphEdit` (D8).
No other function adds a vertex, changes a composition or edits a parent
link; the current bypasses (`insert_atom`, `insert`, `insert_composition`)
are removed (§8.5).

Only two algorithms use the layer to change a graph: insertion
(`insert_match`) and the reader (`read`). They are the authorities for
graph-level correctness: completeness (G2), unique minimal hosts (G5) and
which strings become tokens. The only other user is the import of a
validated oracle snapshot (`from_interchange`, test-api, §8.5), which
reproduces a graph the oracle already built. `GraphEdit` is not a way to
build or repair graphs. It is the small set of **vertex-local** updates the
algorithms share, so that link maintenance, maximality, generation and
validation exist once:

- every operation changes one vertex, plus the parent links of the children
  it adds or removes;
- every operation checks strict preconditions on its inputs and rejects
  anything else (structural checks always, content checks in debug and
  `test-api` builds, where expanded labels exist);
- there is no operation that deletes a vertex, replaces a composition
  wholesale, removes an occurrence without a containing replacement, or
  changes the string of a vertex that has parents.

Failures are prevented, not repaired (D10): a rejected operation means an
algorithm or invariant bug. There is no rollback.

### 8.2 Operations and their users

| Operation | Effect | Preconditions (rejected otherwise) | Insertion | Reader |
|---|---|---|---|---|
| `atom(value) -> Token` | the atom vertex for `value`; existing atoms are returned unchanged (no mutation, no generation bump) | — | — | atoms seen for the first time (reader plan §1.2) |
| `create(occurrences) -> Token` | new composite vertex; children gain parent links | `Composition::new` (sorted, gap-free, no containment, at least 2); children exist with matching widths. Debug: the string is new (G1, `DuplicateContent`), adjacent occurrences agree on their overlap (`OverlapMismatch`), every overlap is a vertex (G4, `OverlapNotVertex`) | every composed interval (§5.2 step 2), `N` last | a new root: `[R@0, t@|R|]` after appending to an adopted token, `maximal([R@0, E@o])` after an expansion (reader plan §3.4) |
| `add_occurrences(v, added)` | `comp(v) := maximal(comp(v) ∪ added)`; contained occurrences move under the added ones (lose the link to `v`) | `v` is composite; each added occurrence lies inside `[0, |v|)` (`OutsideVertex`) and its child exists with a matching width; the width is unchanged (`WidthChange`). Debug: the added token's string equals `v[x, x + |t|)`, G4 for the result | node update (§5.2 step 3) of every split node and of the root | only through `insert_match` (the input root as split node or response root) |
| `extend(v, t@o) -> Token` | `comp(v) := maximal(comp(v) ∪ {t@o})`, `|v| := o + |t|` | `v` is composite and **has no parents** (`HasParents`; no composition contains it, so its width may change); `o ≤ |v| < o + |t|` (`OutsideVertex`). Debug: `t` agrees with `v` on `[o, |v|)`, G4 for the result | — | the owned input root: `Append` (`o = |v|`) and `Expand` (`o = |v| − |P|`) |
| `finish() -> Publication` | validation of touched vertices (debug/test-api) and the record | — | returned in `InsertOutcome::Created` | discarded (the reader keeps tokens only) |

How the algorithms use it:

- **Insertion:** `insert_match` takes the write guard, checks the
  response's generation (D9), opens one `GraphEdit`, and runs the join: for
  each split node in increasing width, `create` for each composed interval,
  then one `add_occurrences` with the node's new tokens. `finish` returns
  the `Publication` (`created` in creation order, `N` last; `edits` per
  changed vertex).
- **Reader:** new atoms through `atom`; each commit (reader plan §3.4)
  opens one `GraphEdit`: `extend` for an owned root, `create` for a new
  root, then `finish`. Matches that are not tokens are inserted by
  `insert_match` before the commit, never by the reader's own edits.

### 8.3 Access (D8)

Rust cannot make a `pub` item visible to exactly two downstream crates:

- `GraphEdit` and `PositionedGraph::edit()` live in
  `context-trace/src/positioned/edit.rs`. They are `pub` only with the
  non-default feature `edit` (`#[doc(hidden)]`) and `pub(crate)` otherwise,
  so context-trace's own import and tests use them without the feature.
- Only `context-insert` and `context-read` enable `context-trace/edit`. A
  guard test (next to the step 1 content-index guard) reads `cargo metadata`
  and fails if any other workspace crate enables it.
- The workspace's public mutation API is `insert_match` (and its wrappers
  `insert`, `insert_next_match`) and `read`. Tests obtain graphs from
  fixtures (`fixture_graphs`, `insert_cases`).

### 8.4 Implementation (`context-trace/src/positioned/edit.rs`)

```rust
impl<G: GraphKind> PositionedGraph<G> {
    /// `pub` with feature `edit` (doc-hidden), `pub(crate)` otherwise.
    pub fn edit(&mut self) -> GraphEdit<'_, G>;
    pub fn generation(&self) -> u64;
}
pub struct GraphEdit<'g, G: GraphKind> {
    graph: &'g mut PositionedGraph<G>,
    publication: Publication,
    touched: BTreeSet<VertexIndex>,
    bumped: bool,
}
impl<G: GraphKind> GraphEdit<'_, G> {
    /// Read access for the join while edits are applied.
    pub fn graph(&self) -> &PositionedGraph<G>;
    /// Existing atom, or a new atom vertex (bumps the generation).
    pub fn atom(&mut self, value: AtomOf<G>) -> Token;
    /// `Composition::new`, child checks, parent links, debug label (the
    /// former `insert_composition`); records the token in
    /// `publication.created`.
    pub fn create(&mut self, occurrences: Vec<Occurrence>) -> Result<Token, PositionedError>;
    /// Same width; records a `VertexEdit`.
    pub fn add_occurrences(&mut self, vertex: VertexIndex, added: Vec<Occurrence>)
        -> Result<(), PositionedError>;
    /// Parentless vertices only; updates the width and the debug label.
    pub fn extend(&mut self, vertex: VertexIndex, occurrence: Occurrence)
        -> Result<Token, PositionedError>;
    /// Validate touched vertices (debug/test-api) and return the record.
    pub fn finish(self) -> Publication;
}
```

- New `PositionedError` variants: `OverlapNotVertex { owner, left, right }`
  (debug/test-api, like `OverlapMismatch`), `HasParents(VertexIndex)`,
  `OutsideVertex { vertex, occurrence }`, `WidthChange { vertex }`.
- `maximal(Vec<Occurrence>) -> Vec<Occurrence>` moves into
  `composition.rs` (shared by `add_occurrences`, `extend` and the join).
  `Composition::crossing(p)` is added next to `covering`/`ending_at` (§4.2).
- Parent links store `(parent, offset)` without the parent's width, so
  `extend` touches only the links of added and removed children. Tokens
  held by callers carry the old width; `extend` returns the new token.
- The generation (D9) is bumped by the **first** mutation of an edit, so a
  dropped `GraphEdit` still invalidates older responses. `finish` does not
  bump it again. Returning an existing atom is not a mutation.
- `finish` validates in debug and test-api builds: structure and links of
  touched vertices and their children, and G4 for touched compositions. G4
  is checked structurally: for adjacent overlapping `L@l`, `R@r`, the
  largest token at `r − l` in `L` (`start_at`) must end at `|L|`.
- No rollback (D10). Edits are applied node by node under the write guard,
  because parents need their children's new tokens. A rejected operation is
  a bug: `insert_match` returns it as `InsertError::Composition` and
  debug-asserts; the gate requires that no case returns an error.

### 8.5 Removed mutation paths and their migration (task 3.1)

| Current | Change |
|---|---|
| `PositionedGraph::insert_atom` (`pub`) | removed; `GraphEdit::atom` |
| `PositionedGraph::insert` (`test-api`) | removed; tests use fixtures or, inside context-trace, `edit()` |
| `PositionedGraph::insert_composition` (`pub(crate)`) | removed; its body becomes `GraphEdit::create` |
| `convert.rs` `build_from_interchange` | builds through one `GraphEdit` (`atom`, `create` bottom-up by width) and calls `validate()` at the end. `from_interchange` becomes `test-api` (its only users are fixture loaders); a production import is decided at cutover (roadmap step 6) |
| `tests/model.rs` | the `ababa` helper builds through `edit()`. `insert_links_parents_and_rejects_incomplete_compositions` asserts the same errors from `GraphEdit::create` (`TooFew`, `Gap`, `Contained`, `UnknownVertex`). `validate_rejects_duplicate_content` and `validate_rejects_overlap_disagreement` can no longer build their graphs through the layer (debug `create` rejects them): they assert the rejection by `create`, and corrupt fields directly for the `validate()` check, as `validate_rejects_broken_reciprocal_links` already does |
| graph.rs doc comment ("the only way to add a composite is `Self::insert`") | points to `GraphEdit` |

The row model (`context-trace::legacy`, context-api, legacy insert and
reader) has its own `Hypergraph` and is not affected.

## 9. Worked examples

### 9.1 A vertex on both paths (`insert_shared_vertex`)

Corpus `acabcabcab, bcabcb`; query `abcabca`. The oracle graph has
`bc = b@0, c@1`, `cab = c@0, a@1, b@2`, `bcab = bc@0, cab@1`,
`cabc = cab@0, bc@2`, `bcabc = bcab@0, cabc@1`, `cabcab = cabc@0, bcab@2`
and the root `acabcabcab = a@0, cabcab@1, bcabc@3, cabcab@4`.

Response: root `acabcabcab`, span `[2, 9)`. Start path
`root → cabcab@1 → cabc@0 → cab@0 → a@1`; end path
`root → cabcab@4 → bcab@2 → cab@1 → a@1`. All positions come from path
edges.

| Node | Positions | Intervals (increasing width) |
|---|---|---|
| `acabcabcab` | `2:S`, `9:E` | `[2,9)` |
| `cabcab` | `1:S`, `5:E` | `[1,5)` overlap, `[0,5)` prefix, `[1,6)` postfix |
| `cabc` | `1:S` | `[1,4)` |
| `bcab` | `3:E` | `[0,3)` |
| `cab` | `1:S`, `2:E` | `[1,2)` (atom `a`, inside a child), `[0,2)`, `[1,3)` |

| Node | Interval → token | Node after |
|---|---|---|
| `cab` | `[0,2)` → `ca = c@0, a@1`; `[1,3)` → `ab = a@0, b@1` | `ca@0, ab@1` |
| `bcab` | `[0,3)` → `bca = bc@0, ca@1` | `bca@0, cab@1` |
| `cabc` | `[1,4)` → `abc = ab@0, bc@1` | `cab@0, abc@1` |
| `cabcab` | `[1,5)` → `abca = abc@0, bca@1`; `[0,5)` → `cabca = cabc@0, abca@1`; `[1,6)` → `abcab = abca@0, bcab@1` | `cabca@0, abcab@1` |
| root | `[2,9)` → `abcabca = abcab@0, bcabc@1, cabca@2` | `a@0, cabcab@1, abcabca@2, cabcab@4` |

New tokens: `ca`, `ab`, `bca`, `abc`, `abca`, `cabca`, `abcab`, `abcabca`.
Each is a prefix or a postfix of `N` (C3); `abca` is both. The result
equals the oracle graph of the extended corpus.

### 9.2 A position off the response paths

Corpus `abcabababcaba` (fixture `complex_overlap`); query `abababc`. The
graph has `ab`, `aba = ab@0, a@2`, `abab = aba@0, ab@2`,
`abcaba = ab@0, c@2, aba@3` and root
`abcabababcaba = abcaba@0, abab@3, abab@5, abcaba@7`.

Response: span `[3, 10)`. Start path `root → abab@3` (the leaf starts at
`o`); end path `root → abcaba@7 → c@2`.

| Node | Positions | Intervals | Resolution |
|---|---|---|---|
| root | `3:S`, `10:E` | `[3,10)` | `abababc = abab@0, abab@2, abc@4` |
| `abcaba` | `3:E` (end path), `3:S` (off path: `abcaba@0` contains `3`) | `[0,3)`, `[3,6)` | `abc = ab@0, c@2` (new); `aba` (inside child `aba@3`) |

`abcaba` becomes `abc@0, aba@3`. Among the root's pieces, `aba@0` comes
from the off-path position, and `maximal` drops it because `abab@0`
contains it. The root becomes `abcaba@0, abababc@3, abcaba@7`.

### 9.3 Both roles at one position (`insert_small_periodic`)

Corpus `ababab`; query `baba`. The graph has `ab = a@0, b@1`,
`abab = ab@0, ab@2` and `ababab = abab@0, abab@2`. Response: root `ababab`,
span `[1, 5)`; start path `ababab → abab@0 → ab@0 → b@1`, end path
`ababab → abab@2 → ab@2 → a@0`.

| Node | Positions | Origin | Intervals |
|---|---|---|---|
| `ababab` | `1:S`, `5:E` | seed | `[1,5)` |
| `abab` | `1:S`, `3:E` | `1` lies inside `abab@0`, `5` inside `abab@2`: two occurrences of one vertex | `[1,3)`, `[0,3)`, `[1,4)` |
| `ab` | `1:S`, `1:E` | `1:S` inside `ab@0`, `3:E` inside `ab@2`: one position with both roles | `[0,1)`, `[1,2)` |

| Node | Interval → token | Node after |
|---|---|---|
| `ab` | `[0,1)` → `a`, `[1,2)` → `b` (inside atom children) | unchanged |
| `abab` | `[1,3)` → `ba = b@0, a@1`; `[0,3)` → `aba = ab@0, ba@1` (`a@2` dropped by `maximal`); `[1,4)` → `bab = ba@0, ab@1` (`b@0` dropped) | `aba@0, bab@1` (`ab@0`, `ab@2` removed, reachable through `aba`, `bab`) |
| root | `[1,5)` → `baba = bab@0, aba@1` | `abab@0, baba@1, abab@2` |

`Publication`: `created = [ba, aba, bab, baba]`; `edits`: `abab` (added
`aba@0, bab@1`, removed `ab@0, ab@2`), `ababab` (added `baba@1`). The overlap
interval `[1,3)` gives `ba`, the overlap of `aba` and `bab` in `abab` and of
`bab` and `aba` in `baba` (G4).

## 10. Legacy mapping

| Legacy | Positioned (rev 3) |
|---|---|
| `materialize(Response)` → `Complete` / `NoExpansion` / `Created` | `insert_match(Response)`, same outcomes |
| `InitInterval { root, cache, end_bound }` | `SplitGraph::from_response` (root, span, path edges) |
| `SplitVertexCache` (positions per vertex) / `SplitPositionCache` (`top`, per-row `(sub_index, inner_offset)`) | `SplitVertex` (positions per vertex) with `Roles`; the child location is the occurrence offset |
| `RootMode` Prefix/Postfix/Infix; root augmentation (inner and wrapper offsets, required ranges) | Roles S/E; derived intervals (target only at the root; postfix, prefix, overlap elsewhere) |
| split queue: push a position into the child containing it, for every row | push a position into every occurrence that strictly contains it |
| join frontier, increasing width | same |
| per-row partitions, range merge, sub-merges, complements, `select` | interval resolution: inside a child, or `maximal(pieces)` |
| `publish`: `insert_pattern`, `replace_in_pattern`, `add_pattern_with_update` | `GraphEdit::create`, `GraphEdit::add_occurrences` |
| `Publication` with row edits | `Publication { created, edits }` |

## 11. Evidence

From `python3 split_positions.py` (prototypes README). Insert cases: for
every v3 fixture (66) and every substring `N` of a vertex string that is not
a vertex, the response is the unique minimal host occurrence, and the
expected graph is `oracle(corpus + [N])`.

| Check | Result |
|---|---|
| Result equals the oracle graph | 1,247 / 1,247 |
| New tokens / created twice / equal to an existing vertex | 2,119 / 0 / 0 (asserted) |
| New tokens that are not `N`, a prefix or a postfix of `N` (C3) | 0 |
| Changed vertices / changed vertices that are not split nodes (J5) | 1,969 / 0 |
| Split nodes (with roots) / non-root positions / from response path edges | 3,397 / 2,632 / 1,969 |
| Cases with positions off the paths / such positions bounding a composed interval | 232 / 0 |
| Max S and E positions per node | 2 and 2 |
| Intervals resolved inside a child (no vertex created) | 1,942 |
| Without overlap intervals (`--no-overlaps`) | 60 mismatches |
| Pushing only along `covering`/`ending_at` (`--paths-only`) | 208 errors (missing crossing pieces) |
| G5 on every fixture; G4 overlapping pairs (`invariants.py`) | 0 exceptions; 0 exceptions in 123 pairs |

## 12. Implementation steps

New code lives at crate roots (`split/`, `join/`, `insert/` in
context-insert, `positioned/edit.rs` in context-trace). Steps are numbered
after the roadmap step and run in order.

### 3.0 Crate setup

- [x] `context-trace/Cargo.toml`: non-default feature `edit` (§8.3, D8).
- [x] `context-insert/Cargo.toml`: depend on `context-trace` (with `edit`)
  and `context-search` without `test-api`. Add `test-api =
  ["context-trace/test-api", "context-search/test-api"]` and enable it in
  `[dev-dependencies]`. Forward it from `legacy-tests` if the legacy tests
  need it.
- [x] Root modules `split/`, `join/`, `insert/`, and `InsertError`.
- [x] `cargo test -p context-insert --features legacy-tests` stays green.

### 3.1 `context-trace`: edit layer, generation, validation, fixtures

- [x] `composition.rs`: `maximal`, `Composition::crossing(p)`.
- [x] `edit.rs`: `GraphEdit` with `atom`, `create`, `add_occurrences`,
  `extend`, `finish` (§8.2, §8.4), `Publication`, `VertexEdit`;
  `PositionedGraph::edit()` (`pub` with feature `edit`, else `pub(crate)`)
  and `generation()`; the new `PositionedError` variants.
- [x] Remove `insert_atom`, `insert` and `insert_composition`; migrate
  `convert.rs` and `tests/model.rs` (§8.5). `from_interchange` becomes
  `test-api`.
- [x] Guard test: no workspace crate other than `context-insert` and
  `context-read` enables `context-trace/edit` (§8.3).
- [x] `validate()`: structural G4 check (`OverlapNotVertex`).
- [x] Insert fixtures (D11, task 3.1.1) and the loader
  `insert_cases() -> Vec<InsertCase { name, base: FixtureGraph, query: String, expected: FixtureGraph }>`
  in `context_trace::positioned::fixtures`.
- **Tests:**
  - each operation keeps `validate()` green and maintains links (added
    children gain `(v, offset)`, removed ones lose it);
  - each rejects inputs outside its preconditions: `create` (`TooFew`,
    `Gap`, `Contained`, `UnknownVertex`; debug: `DuplicateContent`,
    `OverlapMismatch`, `OverlapNotVertex`), `add_occurrences`
    (`OutsideVertex`, `WidthChange`), `extend` (`HasParents`,
    `OutsideVertex`);
  - `atom` returns existing atoms without a generation bump; the generation
    bumps once per mutating edit, also for a dropped edit;
  - `validate()` passes on every fixture (now including G4) and reports
    `OverlapNotVertex` on a graph corrupted by field access;
  - `maximal` (duplicates, containment, partial overlaps kept) and
    `crossing(p)` against a brute-force filter on every fixture composition
    and every `p`.

#### 3.1.1 Insert fixtures (D11)

- **Lane.** `context-read/tests/fixtures/v3/insert/` with its own manifest
  `scenarios.json` (the v3 loaders read only files directly in `v3/`, so
  the lanes do not mix). One entry per insert case:
  `{"name": "<scenario>__<k>", "inputs": corpus + [query]}`. The scenario
  is a v3 manifest entry, the base graph is its v3 fixture and the query is
  the last input. `k` numbers the scenario's cases in `(width, string)`
  order of the query (queries may contain characters not allowed in file
  names).
- **Cases.** For every v3 scenario: every substring (2+ atoms) of a vertex
  string that is not a vertex. Its unique minimal host (G5) is asserted
  while enumerating. 1,247 cases on the current 66 fixtures, plus those of
  the new named scenarios.
- **Generation.** `cargo run -p context-trace --features test-api --example
  insert_manifest` enumerates the cases from the checked-in v3 fixtures
  (fixture strings are test tooling, not algorithm input) and writes the
  manifest. Then
  `./target/release/ngrams --file context-read/tests/fixtures/v3/insert/scenarios.json --out context-read/tests/fixtures/v3/insert`
  writes one oracle graph per case. The context-read README documents both
  commands next to the v3 regeneration.
- **Named scenarios** in the v3 manifest (minimal corpora, the oracle
  derives the repeated children): `insert_small_periodic` (`ababab`; query
  `baba`), `insert_shared_vertex` (`acabcabcab, bcabcb`; `abcabca`),
  `insert_periodic` (`ababababa`; `bababab`), `insert_deep_borders`
  (`baaabababba, abaaabbaabba, aabbaabbaabbaa`; `bbaabbaabb`). Their insert
  cases are generated like all others; the named tests (3.4) pick them by
  query. They are also registered in the `scenarios!` list of
  `context-read/tests/reader_behavior.rs`.
- **Validation** (third lane in `context-read/tests/fixture_validation.rs`):
  exactly one file per manifest entry; each corpus equals its entry's
  inputs and starts with the base scenario's corpus; the manifest equals the
  enumeration from the current v3 fixtures, so a changed or new v3 scenario
  fails until the insert lane is regenerated.

### 3.2 Split graph (`context-insert/src/split/`)

- [x] `SplitGraph`, `SplitVertex`, `Roles` (§4.1).
  `SplitGraph::from_response(&PositionedGraph, &Response)`: seed, path
  edges, propagation (§4.2). Required intervals derived by
  `SplitVertex::intervals(is_root)` (§4.3).
- [x] `test-api` counter: positions that are not path edges.
- **Tests** (every insert case; expectations from strings through the name
  map): every required interval's string is a vertex of the base graph or a
  new vertex of the expected graph, and every new vertex of the expected
  graph is some interval's string; path edges are split edges; the graph is
  unchanged.

### 3.3 Join (`context-insert/src/join/`)

- [x] Frontier, interval resolution, pieces, `maximal`, node update
  through one `GraphEdit` (§5). Debug assertions for J1–J3: a composed
  interval is not inside a child, no composition is created twice, and each
  composition has at least 2 occurrences.
- **Tests:** through 3.4.

### 3.4 `insert_match` (`context-insert/src/insert/`)

- [x] `context-search`: `Response::generation()`, recorded under the read
  guard (part of `Response` equality).
- [x] `InsertOutcome`, `insert_match` under one write guard: generation check,
  `EntireRoot` → `Complete`/`NoExpansion`, otherwise split, join, `finish` →
  `Created`. Wrappers `insert`, `insert_next_match`.
- **Tests:**
  - **Insert gate:** for every insert case, search the query on the base
    graph and check that the root and span equal the brute-force minimal
    host. `insert_match` then gives an empty `diff` against the expected
    graph, and `validate()` passes. No search runs inside `insert_match`;
    `publication.created` has `N` last; the edits list exactly the
    changed vertices.
  - Named cases with the tables of §9.
  - `Complete`/`NoExpansion` with the graph unchanged.
  - `StaleResponse` after an unrelated edit.

### 3.5 Documentation

- [x] `context-insert/README.md`: positioned API section. Mark
  `HIGH_LEVEL_GUIDE.md` as legacy-only and fix its `end_bound` statement
  (`cursor_position()`).
- [x] Roadmap step 3 ticked, INDEX status updated, deviations recorded here.

## 13. Gate

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test --release -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --features legacy-tests
cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app
```

All 3.1–3.4 tests pass on every fixture and insert case. No algorithm calls
`read_interval` or builds content- or composition-keyed structures.

## 14. Decisions

Carried over from revision 2 (confirmed 2026-11-21):

| # | Decision |
|---|---|
| D1 | `insert_match` takes a `Response` only. Its root is the narrowest host, so insertion never searches, never descends to a host and accepts no other input. |
| D2 | Overlaps of occurrences are tokens (G4), checked by `validate()`. |
| D3 | No token is created twice. Revision 3 derives this from G5 (J2) instead of a reuse map. |
| D5 | No multi-host case: a non-token has one minimal host (G5). |

New in revision 3:

| # | Decision |
|---|---|
| D6 | (replaces D4) The split graph stores split positions with roles, not intervals. Positions are pushed into every crossing child (legacy rule). Intervals are derived per vertex. |
| D7 | Insertion handles a single match. Finding matches, including tokens crossing a junction of the read input (legacy root expansion), belongs to the reader ([reader plan](20261122_PLAN_POSITIONED_READER.md) §3.2). No cover merge or seam scan. |

Confirmed 2026-11-23:

| # | Decision |
|---|---|
| D8 | (P1) Vertices and compositions change only through `GraphEdit` (`atom`, `create`, `add_occurrences`, `extend`, `finish`): vertex-local operations with checked preconditions. `insert_atom`, `insert` and `insert_composition` are removed (§8.5). Only insertion and the reader use the layer; outside context-trace it requires the hidden feature `edit`, enabled only by context-insert and context-read (guard test). |
| D9 | (P2) Safeguard against stale responses: `Response` carries the graph generation of its search; `insert_match` rejects a mismatch (`StaleResponse`); the first mutation of a `GraphEdit` bumps the generation. |
| D10 | (P3) No rollback. Failures are prevented by a correct implementation and the invariants; a rejected operation is a bug, returned and debug-asserted, and the gate requires that no case returns an error. |
| D11 | (P4) Expected graphs are generated oracle fixture files: one per insert case in the `v3/insert` lane (task 3.1.1). |

Open (not blocking): skip split positions that are not on a response path
once a proof shows their pieces are always contained (§4.2; 0 of 232
cases need them). This is an optimization, not part of the design.

## 15. Risks

| Risk | Mitigation |
|---|---|
| G5 or G4 fails for some oracle output, or after a reader insertion | Checked on every fixture and insert case; the gate asserts that the root is the brute-force minimal host; formal argument against the frequency rule still open |
| Completeness (G2) is not structurally validated | Oracle diff on every insert case; J4 argument |
| Partial edits after an internal error | D10: prevented by invariants and debug checks; the gate requires no error |
| Fixture volume (about 1,250 small files) | D11: generated by two commands and checked against the enumeration; never edited by hand |
| Removing `insert`/`insert_atom` breaks positioned tests that build graphs by hand | §8.5 migration in task 3.1; the gate's `--no-default-features --features test-api` check covers every positioned crate |
| Cargo feature change breaks legacy builds | 3.0 runs the legacy suite first |

## 16. Implementation notes and deviations (2026-11-23)

Implemented in `context-insert/src/{split,join,insert}/` (tests in
`context-insert/src/tests/`) and `context-search` (`Response::generation`).
The insert gate passes on all 1,379 cases of the insert lane (66 v3
fixtures plus the named scenarios), with no `InsertError`; the Rust split
statistics equal the prototype's on the same cases.

| Measure (1,379 cases) | Value |
|---|---|
| Non-root split positions / from response path edges / off the paths | 3,248 / 2,316 (71.3%) / 932 |
| New tokens | 2,450 (prototype: 2,450) |

The 75% of §4.2 and §11 were measured on the original 1,247 cases.

| # | Plan | Implementation | Why |
|---|---|---|---|
| 1 | §7: `Publication.created` "in creation order (increasing width)" | Creation order is join order: split nodes in increasing `(width, index)`, inside a node intervals in increasing width. It is not globally increasing in width: `sequence__25` (`hypergraph, hyper, graph`; `ergra`) creates `gra` (node `graph`) before `er` (node `hyper`, same width, higher index). `N` is always last. §7 comment corrected | The join order only needs children before parents |
| 2 | §7: `InsertError` has `StaleResponse` and `Composition` | Third variant `Search(SearchError)` | The wrappers `insert`, `insert_next_match` search first and must report `UnknownAtom` / `EmptyPatterns` |
| 3 | §4: `SplitGraph::from_response` | Returns `Option<SplitGraph>`, `None` for `EntireRoot` | `EntireRoot` has no split graph; `insert_match` handles it before splitting |
| 4 | §12 3.2: `test-api` counter | `SplitGraph::stats() -> SplitStats { positions, path_positions }` (`cfg(any(test, feature = "test-api"))`), not a global counter | Per-response, no shared state |
| 5 | §4.2 step 3 assertion | Debug builds record every `(child, position, role)` pushed through `crossing(p)` and assert that each path-edge position is among them; `path_positions` also asserts that every path step except the last strictly contains the border (R4, R5) | — |
| 6 | §6 J2 "a debug assertion checks this" | Debug-only linear scan over the compositions created in this insertion, besides `GraphEdit::create`'s `DuplicateContent` check. Not used by the algorithm | — |
| 7 | §5.2 "that entry exists" | A missing resolved child interval panics (invariant bug); a rejected `GraphEdit` operation is debug-asserted and returned as `InsertError::Composition`. In that case the edit is dropped without `finish` (generation already bumped, earlier operations stay; D10) | D10 |
| 8 | §13 gate | Also runs `cargo test -p context-read` | Requested with the implementation |

