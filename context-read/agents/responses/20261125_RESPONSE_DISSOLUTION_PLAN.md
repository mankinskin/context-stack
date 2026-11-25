---
tags: `#response` `#context-read` `#context-trace` `#reader` `#dissolution` `#staging` `#retention`
summary: Implementation companion to 20261125_PLAN_DISSOLUTION. Explains the current positioned reader loop (segments, expand/forward, root commits, dropped edit records) and why `abcabca` keeps `abc`; summarizes the plan (G6, dissolve/discard, swap-remove, eager `settle`, commit check); gives concrete Rust sketches per task D2–D5 (tagged staging removal, `OverlayHandle::atom`, `raw::Remove`/`Moved`, `GraphEdit::dissolve`/`discard`/`settle`, `Candidates`, reader integration) and the verification steps (unit tests, integration tests, per-step oracle lane, gate).
status: 📎 reference — written against the code at plan finalization, builds and tests not run; some borrow and parent-iteration details are approximate.
date: 2026-11-25
related: ../plans/20261125_PLAN_DISSOLUTION.md
---

# Token dissolution plan: how the reader works today, the code changes, and how results are checked

The plan is in `20261125_PLAN_DISSOLUTION.md`. It fixes one bug: the reader keeps tokens that an early part of the input made frequent but the rest of the input made redundant. The fix adds a cleanup pass called `settle` that runs after every reader step and removes tokens that are no longer needed. I read the plan and the current code. I didn't run any builds or tests. The code below is the concrete form of the plan's §4. A few parent-iteration and borrow details are approximate and the compiler will settle them.

## 1. How the reader works today

The entry point is `read_steps`:

1. **Opening.** `ReadStage::<S>::open(graph)` opens a stage. A stage is a temporary graph layered over the committed graph; all edits of the read go into it. `S` is the staging strategy, which is overlay by default (`staging.rs`).
2. **Splitting the input.** `segments` splits the input into alternating runs of unknown and known atoms. It creates the unknown atoms right away through `GraphEdit::atom`, so today they are **staged** atoms.
3. **Unknown atoms.** Each unknown atom becomes `commit(Span::Append(atom))`.
4. **Known runs.** For each position in a known run:
   - `expand` tries the root's postfixes, largest first, with `insert_next_match([P] ++ rest)`. The first match longer than `P` is inserted and becomes `Expand`, or `ReplaceRoot` if `R[..o] + E` already exists as a token.
   - Otherwise `forward` inserts the longest occurring prefix `t` and returns `ReplaceRoot([R, t])` or `Append(t)`.
5. **Root commit.** `ReadStage::commit` applies the step in one of three ways:
   - **Adopt:** an existing token becomes the root; no edit.
   - **InPlace:** `GraphEdit::extend` grows an editable root (staged, no parents, composite).
   - **NewRoot:** `GraphEdit::create(maximal([R@0, x@o]))`.
6. **Dropped records.** Both the insertion's `Publication` (the edit record from `InsertOutcome::Created`) and the commit's `edit.finish()` result are **thrown away**.
7. **Finish.** `finish()` calls `handle.commit(root)`, and the overlay (`handle.rs`) pushes the staged vertices into the committed graph via `commit_into`.

**The bug.** A token created early in the read is never removed again. In `abcabca`, `abc` is a valid token after `abcabc`. The final `a` creates `abca` at offsets 0 and 3, which leaves `abc` with only one parent occurrence. The oracle graph (the expected graph from the n-gram reference implementation) has no `abc`, but the reader commits it anyway.

This affects 9 ignored scenarios in `reader_behavior.rs` (`#[ignore = "provisional retention …"]`) and one entry in `READ_DEVIATIONS` (`insert_deep_borders__1`). No bounded look-ahead can fix this; a decision made earlier in the read has to be revised.

## 2. What the plan does

**Retention invariant G6.** Every committed vertex that is neither an atom nor an input must have at least 2 parent occurrences. G6 holds on all 2339 oracle graphs.

**Two removal operations** (only for staged composites that are not the root):
- **`dissolve(t)`** (exactly one parent occurrence `(U, o)`):
  - `U`'s composition becomes `maximal(comp(U) − t@o ∪ comp(t)+o)`.
  - `t`'s children are moved up into `U`, then `t` is removed.
  - `U` may be committed; then only its composition shape changes, through its copy-on-write delta.
- **`discard(t)`** (no parent occurrence): unlink `t`'s children and remove `t`.

**Swap-remove.** Removal takes effect at once: the last staged slot moves into the freed slot, and every reference to it is renamed in the same call.

**`settle`** runs after every step. It uses a worklist built from the step's edit records:
- children of `removed` occurrences,
- newly created tokens,
- the previous root, if the root changed.

Each candidate is kept, dissolved or discarded depending on its parent count, and cascades are added back to the worklist. This is exact because a parent count can only drop through a recorded `removed` occurrence. The result does not depend on processing order.

**Reader decisions do not change.** Appendix A (lemmas L1–L6 and RM) shows that dissolution changes no forward, probe or lookup decision. At most an insertion that would have found a complete token creates one again instead.

**Commit check.** `StageHandle::commit` panics if any staged composite other than the root has fewer than 2 parents. This is the only G6 check at runtime.

**Order of work:** D2 → D3 → D4 → D5 → D6. D1b (proofs) runs in parallel. D7 (early commit) comes last and still has one open decision (E-I). Only D5 changes behavior.

## 3. Code changes

### D2: remove tagged staging (no behavior change)

- **Delete:** `context-trace/src/positioned/stage/tagged/`, `tests/stage/tagged.rs`, and `context-read/src/staging.rs`.
- **`stage/mod.rs`:** `Translation` becomes a plain table:
  ```rust
  pub struct Translation(Vec<CommittedIndex>);
  pub fn index(&self, index: StageIndex) -> CommittedIndex {
      match index.split() {
          Either::Left(c) => c,
          Either::Right(s) => self.0[s.slot()],
      }
  }
  ```
  Remove `Map`, `untag` and `pub mod tagged;`.
- **context-trace cleanup:**
  - Remove `PositionedGraph::stage: Option<StageId>` and `PositionedVertex::state`; committed `VertexRead::state()` returns `VertexState::Committed`.
  - Remove the `state: VertexState::Committed` initializer in `staging.rs`.
  - Remove `assert!(base.stage.is_none(), …)` in `OverlayHandle::open`.
  - Update the allowlists in `tests/guard.rs` and remove `StageIndex::tagged`.
- **`read.rs` in context-read:**
  ```rust
  pub fn read(graph: &PositionedGraphRef, input: impl IntoIterator<Item = char>) -> Option<Token> {
      read_steps(graph, input, |_| {})
  }
  fn read_steps(graph, input, observe) -> Option<Token> { let mut root = ReadStage::open(graph); … }
  ```
- **`root.rs`:**
  ```rust
  pub(crate) struct ReadStage<'g> { pub(crate) handle: OverlayHandle<'g>, root: Option<Token<StageIndex>> }
  // open: handle: OverlayHandle::open(graph)
  ```
  The test alias becomes `type Stage<'g> = ReadStage<'g>;`.
- **`lib.rs`:** export only `Commit, Step, StepKind, read, read_traced`.
- **`reader_behavior.rs`:**
  - Remove the `strategies!` macro, `dump` and `strategies_commit_identical_graphs`.
  - Generate tests with `with_scenarios!(scenarios)` at top level.
  - Make `assert_scenario`, `read_case` and `assert_read_gate` non-generic, calling `context_read::read`.
  - Keep one `#[test] fn read_gate()`.
- **Benches:** drop the `_tagged` variants and the generics.

### D3: atoms committed at once, plus swap-remove (no behavior change)

New atoms are written to the committed graph immediately (`handle.rs`):

```rust
impl<'g, G: GraphKind> OverlayHandle<'g, G> {
    pub fn atom(&mut self, value: AtomOf<G>) -> Result<Token<StageIndex>, CommitError> {
        if let Some(&index) = self.base.read().atoms.get(&value) {
            return Ok(Token::new(StageIndex::lift(index), 1));
        }
        let mut base = self.base.write();
        let stage = self.stage.get_mut();
        if base.generation != stage.base_generation {
            return Err(CommitError::BaseChanged { opened: stage.base_generation, now: base.generation });
        }
        let mut edit = base.edit();
        let token = edit.atom(value);
        edit.finish();
        stage.base_generation = base.generation; // the commit check still catches foreign edits
        Ok(Token::new(StageIndex::lift(token.index), 1))
    }
}
```

Supporting changes:
- **`StagingGraph::atoms`** is removed.
- **`OverlayView::atom`** looks up `self.base.atoms` only.
- **`raw::Write::push`** with `VertexNode::Atom` and **`register_atom`** on the overlay both `panic!("atoms are committed through OverlayHandle::atom")`.
- **`commit_into`** drops the `VertexNode::Atom` arm.
- **`segments`** works on the handle directly, with no open view or `GraphEdit`:
  ```rust
  pub(crate) fn segments(handle: &mut OverlayHandle<'_>, chars: impl IntoIterator<Item = char>) -> Vec<Segment> {
      for c in chars {
          let known = handle.graph().atom(&c).map(|a| a.to_token());
          let token = known.unwrap_or_else(|| handle.atom(c).unwrap_or_else(|e| panic!("atom: {e}")));
          … // same run logic
      }
  }
  ```
  In the segment tests, `x` becomes `Committed`, `generation().stage` stays 0, and the base generation grows.

Swap-remove adds a new raw primitive in `store/write.rs` and a `Moved` type in `stage/mod.rs`:

```rust
// store/write.rs, inside `pub(crate) mod raw`
pub trait Remove: Write {
    fn remove(&mut self, seal: Seal, index: StageIndex) -> Option<Moved>;
}

// stage/mod.rs
pub trait Stage: GraphWrite<Index = StageIndex> + raw::Remove { /* staged(): "slot order" */ }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moved { pub from: StageIndex, pub to: StageIndex }
impl Moved {
    pub fn apply(self, i: StageIndex) -> StageIndex { if i == self.from { self.to } else { i } }
}
```

The overlay implementation (`view.rs`):

```rust
impl<G: GraphKind> raw::Remove for OverlayView<'_, G> {
    fn remove(&mut self, _: Seal, t: StageIndex) -> Option<Moved> {
        let slot = t.staged().expect("only staged vertices are removed").slot();
        let stage = &mut *self.stage;
        debug_assert!(stage.vertices[slot].parents.is_empty());
        let last = stage.vertices.len() - 1;
        stage.vertices.swap_remove(slot);
        stage.generation += 1;
        if slot == last { return None; }
        let (from, to): (StageIndex, StageIndex) = (StagedIndex::new(last).into(), t);
        let moved = &stage.vertices[slot];
        // 1. children of the moved vertex: rename the parent key from → to
        if let VertexNode::Composite(c) = &moved.node {
            for o in c.occurrences() {
                let links = match o.child.index.split() {
                    Either::Right(s) => &mut stage.vertices[s.slot()].parents,
                    Either::Left(c) => &mut stage.deltas.get_mut(&c).expect("delta").added,
                };
                if let Some(offs) = links.remove(&from) { links.insert(to, offs); }
            }
        }
        // 2. parents of the moved vertex: rename the child in their composition
        for (&p, _) in moved.parents.clone().iter() {
            let comp = match p.split() {
                Either::Right(s) => match &mut stage.vertices[s.slot()].node {
                    VertexNode::Composite(c) => c,
                    _ => unreachable!(),
                },
                Either::Left(c) => stage.deltas.get_mut(&c).and_then(|d| d.composition.as_mut()).expect("delta composition"),
            };
            *comp = comp.clone().map_index(|i| if i == from { to } else { i });
        }
        Some(Moved { from, to })
    }
}
```

(The real implementation has to split the borrows: collect the occurrences and parents first, then mutate.)

### D4: dissolution, `settle` and checks (no behavior change)

New file `context-trace/src/positioned/edit/dissolve.rs`:

```rust
impl<S: Stage + ?Sized> GraphEdit<'_, S> {
    fn removable(&self, t: StageIndex, root: StageIndex, want: usize) -> Result<(), Error<S>> {
        if !t.is_staged() { return Err(PositionedError::NotStaged(t)); }
        self.composite(t)?; // Atom(t) error for atoms
        if t == root { return Err(PositionedError::IsRoot(t)); }
        let count = expect_vertex(&*self.graph, t)?.parent_count();
        if count != want { return Err(PositionedError::ParentCount { vertex: t, count }); }
        Ok(())
    }

    pub fn dissolve(&mut self, t: StageIndex, root: StageIndex) -> Result<Removal, Error<S>> {
        self.removable(t, root, 1)?;
        let (_, children) = self.composite(t)?;
        let (u, o) = /* the only (parent, offset) of t */;
        let (_, old) = self.composite(u)?;
        let mut union: Vec<_> = old.iter().filter(|x| !(x.child.index == t && x.offset.0 == o)).copied().collect();
        union.extend(children.iter().map(|c| Occurrence::new(o + c.offset.0, c.child)));
        let comp = Composition::new(maximal(union)).expect("T1: dissolution keeps a valid composition");
        self.mutate();
        self.recompose(u, &old, comp);            // unlinks t@o, links the moved children
        for c in &children { self.unlink(t, c); }
        Ok(self.removed(t, Some(u), children))
    }

    pub fn discard(&mut self, t: StageIndex, root: StageIndex) -> Result<Removal, Error<S>> {
        self.removable(t, root, 0)?;
        let (_, children) = self.composite(t)?;
        self.mutate();
        for c in &children { self.unlink(t, c); }
        Ok(self.removed(t, None, children))
    }

    /// Swap-remove plus bookkeeping: touched / edits / created drop t and rename via `moved`.
    fn removed(&mut self, t: StageIndex, host: Option<StageIndex>, children: Vec<Occurrence<StageIndex>>) -> Removal {
        let moved = self.graph.remove(SEAL, t);
        self.touched.remove(&t);
        self.edits.remove(&t);
        self.created.retain(|tok| tok.index != t);
        let ren = |i| moved.map_or(i, |m| m.apply(i));
        self.touched = self.touched.iter().map(|&i| ren(i)).collect();
        self.edits = std::mem::take(&mut self.edits).into_iter().map(|(k, mut e)| {
            e.vertex = ren(k);
            for o in e.added.iter_mut().chain(e.removed.iter_mut()) { o.child.index = ren(o.child.index); }
            (ren(k), e)
        }).collect();
        for tok in &mut self.created { tok.index = ren(tok.index); }
        Removal { removed: t, host: host.map(ren), children: children.iter().map(|c| ren(c.child.index)).collect(), moved }
    }

    pub fn settle(&mut self, candidates: Candidates, root: &mut Token<StageIndex>) -> Settled {
        let mut work: Vec<StageIndex> = candidates.0; work.sort(); work.dedup();
        let mut s = Settled { candidates: work.len(), ..Default::default() };
        while let Some(t) = work.pop() {
            if !t.is_staged() || t == root.index { continue; }
            let r = match self.graph.vertex(t).map(|v| v.parent_count()) {
                Some(n) if n >= 2 => continue,
                Some(1) => { s.dissolved += 1; self.dissolve(t, root.index) }
                Some(0) => { s.discarded += 1; self.discard(t, root.index) }
                _ => continue,
            }.unwrap_or_else(|e| panic!("settle: {e}"));
            if r.host == Some(root.index) { s.into_root += 1; }
            work.retain(|&i| i != t);
            if let Some(m) = r.moved { for i in &mut work { *i = m.apply(*i); } root.index = m.apply(root.index); }
            work.extend(r.children);
        }
        #[cfg(any(debug_assertions, feature = "test-api"))]
        for staged in self.graph.staged() {
            let i: StageIndex = staged.into();
            let v = self.graph.vertex(i).unwrap();
            assert!(i == root.index || v.node().composite().is_none() || v.parent_count() >= 2,
                "invariant (S) violated at {i}");
        }
        s
    }
}

#[derive(Clone, Debug, Default)]
pub struct Candidates(Vec<StageIndex>);
impl Candidates {
    pub fn record(&mut self, p: &Publication<StageIndex>) {
        self.0.extend(p.edits.iter().flat_map(|e| e.removed.iter().map(|o| o.child.index)));
        self.0.extend(p.created.iter().map(|t| t.index));
    }
    pub fn push(&mut self, v: StageIndex) { self.0.push(v); }
}
// Removal { removed, host, children, moved } and Settled { candidates, dissolved, discarded, into_root } as in plan §4.3
```

Other D4 changes:
- **`edit/mod.rs`:** add `mod dissolve;` and re-export `Candidates, Removal, Settled, Moved` from `positioned`.
- **`model/error.rs`:** new variants `ParentCount { vertex: I, count: usize }` and `IsRoot(I)`, each with a `Display` arm.
- **Commit check** in `OverlayHandle::commit`, after the generation check and before `commit_into`:
  ```rust
  for (slot, v) in stage.vertices.iter().enumerate() {
      let i: StageIndex = StagedIndex::new(slot).into();
      let count: usize = v.parents.values().map(|o| o.len()).sum();
      if i != root.index && matches!(v.node, VertexNode::Composite(_)) && count < 2 {
          panic!("invalid vertex set: staged {i} has {count} parent occurrences");
      }
  }
  ```
- **`PositionedGraph::validate_retention(inputs)`** (`test-api`): every composite not in `inputs` must have `parent_count() >= 2`, else `ParentCount`.
- **Test helpers** (`test-api`): `store_interchange(&impl GraphRead)` (`to_interchange` delegates to it) and `validate_store(&impl GraphRead)`, which runs `validate_vertex` on every vertex and `check_overlap_closure` on every composite.

### D5: reader integration (fixes the bug)

- **`forward.rs`:** return the publication alongside the span:
  ```rust
  ) -> (Span, Option<Publication<StageIndex>>) {
      let (token, publication) = match insert_next_match(&mut *handle, rest.to_vec()) {
          Ok(InsertOutcome::Created { token, publication, .. }) => (token, Some(publication)),
          Ok(outcome) => (outcome.token(), None),
          Err(InsertError::Search(SearchError::SingleIndex(atom))) => (atom, None),
          Err(e) => panic!("forward match failed: {e}"),
      };
      … (span, publication)
  ```
- **`expand.rs`:** `Accepted` gains `publication: Option<Publication<StageIndex>>`. A rejected probe must not produce one (`debug_assert!(matches!(outcome, …not Created))` before `continue`). Because `Accepted` now holds a `Vec`, drop its `Copy` derive.
- **`root.rs`:**
  ```rust
  pub(crate) fn commit(&mut self, span: Span) -> (Commit, Option<Publication<StageIndex>>) {
      // Adopt paths: return (Commit::Adopt, None)
      … let publication = edit.finish(); … (kind, Some(publication))
  }
  pub(crate) fn settle(&mut self, candidates: Candidates) -> Settled {
      let Some(mut root) = self.root else { return Settled::default() };
      let mut view = self.handle.stage();
      let mut edit = GraphEdit::new(&mut view);
      let settled = edit.settle(candidates, &mut root);
      edit.finish();
      drop(view);
      self.root = Some(root);
      settled
  }
  ```
- **`read.rs`, the per-step closure:**
  ```rust
  let mut commit = |root: &mut ReadStage<'_>, kind, span, inserted: Option<Publication<StageIndex>>| {
      let previous = root.token();
      let (commit, committed) = root.commit(span);
      let mut candidates = Candidates::default();
      for p in inserted.iter().chain(committed.iter()) { candidates.record(p); }
      if let Some(prev) = previous.filter(|p| Some(p.index) != root.token().map(|t| t.index)) {
          candidates.push(prev.index);
      }
      let settled = root.settle(candidates);
      observe(StepView { step: Step { kind, commit, root: width, dissolved: settled.dissolved, discarded: settled.discarded }, handle: &root.handle, root: root.token() });
  };
  ```
  - Unknown atoms pass `None`; expansion passes `accepted.publication`; forward passes its publication.
  - `Step` gains `pub dissolved: usize, pub discarded: usize`.
  - `StepView` is `pub(crate)` for the per-step lane; `read_traced` maps it to `Step`.
- **Tests:**
  - Remove the 9 `#[ignore = "provisional retention …"]` attributes and `READ_DEVIATIONS`.
  - `assert_read_gate` expects `failed.is_empty()`.
  - `assert_scenario` calls `validate_retention(returned_tokens_so_far)` after every read.
  - `read_case` builds the base with `from_interchange_named` and passes the base corpus plus the returned root as inputs.
  - `step(..)` sets `dissolved: 0, discarded: 0`.
  - New `context-read/src/tests/steps.rs`.

### D6 and D7

- **D6:** run `cargo bench -p context-read --no-default-features --features test-api` and record the numbers in §6 of the plan. No optimization is part of this plan.
- **D7 (early commit):** the rule is final, but the interface is still open (E-I: a stability mark on a stable slot prefix vs. physically committing during the read). Implement it last, after the D1b proofs.

### Walkthrough: `abcabca` after D5 (plan §3.5)

1. Forward `abc` creates the root S2 `[S1@0, S1@3]`, where S1 is `abc`.
2. The probe `[S1]++"a"` creates S3 `abca` inside S2. The insert record shows S2 removed `S1@0`.
3. The `Expand` commit makes S2 `[S3@0, S3@3]`. The commit record shows S2 removed `S1@3`.
4. `settle` gets the candidates {S3, S1}:
   - S3 has p = 2 and is kept.
   - S1 has p = 1 (parent `S3@0`), so it is dissolved into S3, which becomes `[a@0, b@1, c@2, a@3]`.
   - The swap-remove moves S3 into S1's slot.
5. `commit` passes the G6 check, and the result equals the oracle graph.

## 4. How the results are verified

1. **Unit tests** in context-trace (new `positioned/tests/stage/dissolve.rs`, oracle-consistent fixtures):
   - **D3 `atom`:** commits immediately; detects a base changed by someone else (`BaseChanged`).
   - **D3 swap-remove:** renames the moved vertex in staged and committed children and parents, passes `validate_store`, and returns `None` when the last slot is removed.
   - **D4 preconditions:** each error (`NotStaged`, `Atom`, `IsRoot`, `ParentCount`) leaves the graph unchanged.
   - **D4 postconditions:** the host's new composition equals the `maximal(…)` result, and the returned `Removal` is correct.
   - **D4 `Candidates`:** an occurrence removed and re-added in one edit is not a candidate.
   - **D4 `settle`:** reproduces the walkthrough; handles a cascade; renames the root when the root is the moved vertex.
   - **D4 commit panic:** `#[should_panic]` on a staged composite with p < 2.
   - **D4 `validate_retention`:** passes on all fixtures and fails on a graph with a non-input composite of p = 1.
2. **Integration tests** in `reader_behavior.rs`: all 94 scenarios with no ignores, `read_gate` with 0 deviations over 140 cases, and `validate()` plus `validate_retention` after every read. `fixture_validation.rs` stays unchanged; it checks G6 on all oracle graphs.
3. **Per-step oracle lane** (`src/tests/steps.rs`), after every step, post-`settle`, on the stage view:
   - `validate_store` passes;
   - invariant (S) holds;
   - `store_interchange(view)` equals the oracle graph (`ngrams::graph::parse_corpus`) of the earlier inputs plus the consumed prefix. Atoms not yet consumed are ignored.

   The prototype matched 701 of 701 steps.
4. **Gate after every task (§5.4):** `cargo check` and `cargo test` with `--no-default-features --features test-api`, the release `insert_gate`, the `legacy-tests` suites, and the workspace build excluding `context-editor-kernel` and `sandbox-app` (winit does not build here).
5. **Confirming the fix:**
   ```sh
   cargo test -p context-read --no-default-features --features test-api --test reader_behavior repeat_abcabca
   ```
   This passes without `--ignored`, and the result has `abca = [a@0, b@1, c@2, a@3]` and no `abc`. The same must hold for the other 8 scenarios and for `insert_deep_borders__1`.

The plan counts as done when every scenario, every read-lane case and every intermediate step equals its oracle graph.