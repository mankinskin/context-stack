# Failing Tests Tracker

> Last updated: 2026-03-15
> Run command: `cargo test -p context-cli --test cli_integration`
> Commit snapshot: manual integration test session

## Summary

| Status | Count |
|--------|-------|
| ✅ Passing | 44 |
| ❌ Failing | 9 |
| ⏭️ Ignored | 22 |
| **Total** | **75** |

---

## Root Causes

| ID | Root Cause | Crate | Affected Tests |
|----|-----------|-------|----------------|
| RC-1 | `insert_sequence` calls `insert_next_match` only **once** — first atom matched via `NoExpansion`, compound tokens never created; `already_existed=true` and `width=1` for every multi-char insert | `context-api` / `context-insert` | `read_known_pattern`, `read_produces_decomposition_tree`, `read_text_output`, `dedup_insert_then_read`, `file_read_basic`, all `skill3_exp_*` ignored tests, all `oracle_*` ignored tests |
| RC-2 | `ReadCtx::read_sequence` returns `None` after a prior insert has modified the graph state | `context-read` | `read_sequence_after_insert`, `dedup_read_sequence_finds_existing`, `dedup_multiple_reads_same_result` |
| RC-3 | Repeat/overlap cursor advancement broken — `edge_repeated_single_char` panics or returns first atom only; all-same-char inputs unsupported | `context-read` | `edge_repeated_single_char`, `oracle_aa`, `skill3_exp_m_repeated_char_known_failing` |

---

## RC-1 Mechanism (identified via Skill 3 exploration)

`insert_sequence` converts text to atom tokens, then calls `insert_next_match([atom₀, atom₁, …])`
**once**. On a fresh graph the first call matches only `atom₀` (the first character) and returns
`NoExpansion` — meaning "I matched a complete token at position 0 but the query extends beyond it;
advance the cursor." Because `already_existed = !outcome.is_expanded()` and `NoExpansion` is not
`Created`, the function returns `already_existed=true` and `token.width=1` (the first atom) for
**every** multi-character insert.

**Fix needed:** Add an outer loop to `WorkspaceManager::insert_sequence` that repeatedly calls
`insert_next_match` with the remaining atoms, collecting segment tokens, until the full input is
consumed. The collected segments are then wrapped into a root token. The outer loop logic already
exists inside `ReadCtx` — it needs to be lifted to / duplicated in the `insert_sequence` path.

**Acceptance criteria (from `skill3_exp_*` tests):** Once the outer loop is in place, all
`#[ignore = "RC-1: …"]` tests must turn green when run with `--include-ignored`.

**Unaffected today:** atom auto-creation, atom deduplication, graph validation, `ReadPattern` on
single atoms, `ReadAsText` on single atoms, `read_sequence` on fresh sequences (no prior insert),
`GetStatistics`, `ValidateGraph`.

---

## RC-2 Mechanism

After `insert_sequence` writes atoms and a (broken, width=1) token into the graph, a subsequent
`ReadCtx::read_sequence` call on the same text returns `None`. The root cause appears to be that
`ReadCtx` resolves atoms from the graph at construction time and the atom/pattern indices written
by `insert_sequence` interfere with the `read_sequence` traversal. The exact interaction has not
yet been fully traced.

**Fix dependency:** RC-2 may partially resolve when RC-1 is fixed (because the graph state after
a correct insert differs significantly from the current broken state). RC-2 should be re-evaluated
after RC-1 is resolved before investing in a separate fix.

---

## RC-3 Mechanism

When `read_sequence` (or the future outer-loop `insert_sequence`) encounters a repeated character
(e.g. `"aaaa"`), the cursor advancement logic fails to advance past the first atom. The pattern
accumulator gets stuck and either returns a width-1 result or panics. This is the
`append_to_pattern` / cursor advancement bug described in the parent plan Phase 3 foundation fixes.

**Fix dependency:** Blocked on `context-read` cursor advancement redesign (separate plan).

---

## Failure Details

### ❌ `integration::basic_read_tests::read_known_pattern`

- **Root Cause:** RC-1
- **Test file:** `tests/integration/basic_read_tests.rs`
- **Expected:** Insert "abc" → `ReadPattern` on root → `read.text == "abc"`, children non-empty
- **Actual:**
  ```
  assertion `left == right` failed
    left: "a"
   right: "abc"
  ```
  The root token produced by `insert_sequence` is only the first atom ("a", width=1).
- **Fix Plan:** Add outer loop to `insert_sequence` (RC-1 fix)

---

### ❌ `integration::basic_read_tests::read_produces_decomposition_tree`

- **Root Cause:** RC-1
- **Test file:** `tests/integration/basic_read_tests.rs`
- **Expected:** Insert "hello" → `ReadPattern` on root → `read.text == "hello"`, `root.width == 5`
- **Actual:**
  ```
  assertion `left == right` failed
    left: "h"
   right: "hello"
  ```
  Root is the first atom "h" (width=1). No compound token is ever created.
- **Fix Plan:** Same as RC-1

---

### ❌ `integration::basic_read_tests::read_text_output`

- **Root Cause:** RC-1
- **Test file:** `tests/integration/basic_read_tests.rs`
- **Expected:** Insert "hello" → `ReadAsText(root.index)` → `"hello"`
- **Actual:**
  ```
  assertion `left == right` failed
    left: "h"
   right: "hello"
  ```
  `ReadAsText` on the (incorrectly width=1) root returns only the first character.
- **Fix Plan:** Same as RC-1

---

### ❌ `integration::basic_read_tests::read_sequence_after_insert`

- **Root Cause:** RC-2
- **Test file:** `tests/integration/basic_read_tests.rs`
- **Expected:** Insert "abc" → `read_sequence("abc")` returns `PatternReadResult` with `text == "abc"`
- **Actual:**
  ```
  read_sequence("abc") failed: internal read error: read_sequence returned None for text of length 3
  ```
  `ReadCtx::read_sequence` returns `None` when the graph already has entries from a prior insert.
- **Fix Plan:** Investigate `ReadCtx` atom resolution after graph is modified by `insert_sequence`;
  re-evaluate after RC-1 fix is applied.

---

### ❌ `integration::dedup_tests::dedup_insert_then_read`

- **Root Cause:** RC-1
- **Test file:** `tests/integration/dedup_tests.rs`
- **Expected:** Insert "hello" → `ReadAsText(root.index)` → `"hello"`
- **Actual:**
  ```
  assertion `left == right` failed
    left: "h"
   right: "hello"
  ```
  Same truncation as `read_text_output`.
- **Fix Plan:** Same as RC-1

---

### ❌ `integration::dedup_tests::dedup_read_sequence_finds_existing`

- **Root Cause:** RC-2
- **Test file:** `tests/integration/dedup_tests.rs`
- **Expected:** Insert "hello" → `read_sequence("hello")` returns `text == "hello"`, `root.width == 5`
- **Actual:**
  ```
  read_sequence("hello") failed: internal read error: read_sequence returned None for text of length 5
  ```
- **Fix Plan:** Same as RC-2

---

### ❌ `integration::dedup_tests::dedup_multiple_reads_same_result`

- **Root Cause:** RC-2
- **Test file:** `tests/integration/dedup_tests.rs`
- **Expected:** Insert "test" → two calls to `read_sequence("test")` return identical results
- **Actual:**
  ```
  read_sequence("test") failed: internal read error: read_sequence returned None for text of length 4
  ```
  First call fails immediately.
- **Fix Plan:** Same as RC-2

---

### ❌ `integration::edge_case_tests::edge_repeated_single_char`

- **Root Cause:** RC-3
- **Test file:** `tests/integration/edge_case_tests.rs`
- **Expected:** `read_sequence("aaaa")` → `text == "aaaa"`, `root.width == 4`
- **Actual:**
  ```
  assertion `left == right` failed
    left: "a"
   right: "aaaa"
  ```
  Cursor gets stuck; only the first atom is returned (width=1).
- **Fix Plan:** `context-read` cursor advancement redesign (RC-3 fix, tracked separately)

---

### ❌ `integration::file_input_tests::file_read_basic`

- **Root Cause:** RC-1
- **Test file:** `tests/integration/file_input_tests.rs`
- **Expected:** `ReadFile("hello world")` → `read.text == "hello world"`
- **Actual:**
  ```
  assertion `left == right` failed
    left: "helo wrd"
   right: "hello world"
  ```
  `ReadFile` internally calls `read_sequence`. Because `read_sequence` on a fresh graph works
  (no prior insert), the result is not `None` — but the outer loop is still absent, so it only
  collects the first distinct atom per position. Repeated characters ('l', 'l', 'o', ' ', 'o')
  cause premature deduplication: "helo wrd" (8 chars) instead of "hello world" (11 chars).
  This is a distinct symptom from the pure RC-1 `insert_sequence` truncation, but shares the
  same root cause: the outer accumulation loop is missing.
- **Fix Plan:** Same as RC-1 — once `read_sequence` / `insert_sequence` correctly loops over
  `insert_next_match`, repeated characters will be handled properly and `collect_leaf_text` will
  reconstruct the full string.

---

## Ignored Tests (22)

### RC-1 Blocked — `skill3_exp_*` (9 tests)

These tests document the **intended** post-fix behaviour of `insert_sequence` and will become
green once the outer loop is added:

| Test | Scenario |
|------|----------|
| `skill3_exp_a_fresh_insert_is_created` | First insert → `already_existed=false`, `width=N` |
| `skill3_exp_b_second_insert_is_complete` | Second insert → same index, `already_existed=true` |
| `skill3_exp_c_compound_from_known_pieces` | Insert "hel"+"lo"+"hello" → "hello" Created from parts |
| `skill3_exp_e_known_prefix_then_new_suffix` | Insert "hello" then "helloworld" — NoExpansion then Created |
| `skill3_exp_f_both_pieces_known_before_compound` | Insert "hello"+"world"+"helloworld" — compound from known pieces |
| `skill3_exp_g_idempotent_multiple_complete_calls` | 4× same text — first Created, rest Complete |
| `skill3_exp_i_minimum_length_insert` | 2-char insert → Created, width=2 |
| `skill3_exp_k_dungeon_analogy_compound` | Dungeon scenario — "dungeon run" + "dungeon dungeon" |
| `skill3_exp_l_bulk_created_then_complete` | 5 sequences Created then Complete |

### RC-1 Blocked — `oracle_*` end-to-end ngrams oracle tests (11 tests)

These tests compare the context-read graph against the ngrams oracle graph. All blocked on RC-1.
Un-ignoring any of these after the RC-1 fix gives an immediate structural correctness signal.

| Test | Input | Notes |
|------|-------|-------|
| `oracle_ab` | `"ab"` | Minimum 2-char, all-distinct |
| `oracle_abab` | `"abab"` | Repeated bigram |
| `oracle_abcabc` | `"abcabc"` | Repeated trigram |
| `oracle_abcbcd` | `"abcbcd"` | Adjacent overlap (shared "bc") |
| `oracle_aabbaabb` | `"aabbaabb"` | Nested repetition |
| `oracle_ababab` | `"ababab"` | Longer binary repetition |
| `oracle_abcab` | `"abcab"` | Partial overlap at end |
| `oracle_aabaa` | `"aabaa"` | Complex repetition |
| `oracle_abcdabc` | `"abcdabc"` | Prefix repeat, length 7 |
| `oracle_slow_abcabcabc` | `"abcabcabc"` | Triple repetition, length 9 — also slow |
| `oracle_slow_abababab` | `"abababab"` | Long binary repeat, length 8 — also slow |

### RC-3 Blocked (2 tests)

| Test | Scenario |
|------|----------|
| `oracle_aa` | `"aa"` — all-same-char, RC-3 boundary case |
| `skill3_exp_m_repeated_char_known_failing` | `"aaaa"` → Created, width=4, 1 atom |

---

## Passing Tests (44)

### Common — Graph Compare Machinery (9/9 ✅)

- `common::graph_compare::tests::compare_concatenation_violation` ✅
- `common::graph_compare::tests::compare_identical_maps_is_ok` ✅
- `common::graph_compare::tests::compare_root_missing_from_oracle` ✅
- `common::graph_compare::tests::compare_unverified_label_is_only_info` ✅
- `common::graph_compare::tests::compare_width_mismatch` ✅
- `common::graph_compare::tests::label_map_atoms_only` ✅
- `common::graph_compare::tests::label_map_compound_token` ✅
- `common::graph_compare::tests::label_map_multiple_patterns` ✅
- `common::graph_compare::tests::report_display_smoke_test` ✅

### Category 1: Atom Management (6/6 ✅)

- `integration::atom_tests::atom_create_basic` ✅
- `integration::atom_tests::atom_create_duplicate` ✅
- `integration::atom_tests::atom_create_unicode` ✅
- `integration::atom_tests::atom_auto_create_on_insert` ✅
- `integration::atom_tests::atom_get_by_index` ✅
- `integration::atom_tests::atom_list_all` ✅

### Category 2: Basic Read (4/8 ✅ — 4 failing)

- `integration::basic_read_tests::read_empty_sequence_returns_error` ✅
- `integration::basic_read_tests::read_sequence_single_char` ✅
- `integration::basic_read_tests::read_sequence_text` ✅
- `integration::basic_read_tests::read_single_atom` ✅
- `integration::basic_read_tests::read_known_pattern` ❌ (RC-1)
- `integration::basic_read_tests::read_produces_decomposition_tree` ❌ (RC-1)
- `integration::basic_read_tests::read_sequence_after_insert` ❌ (RC-2)
- `integration::basic_read_tests::read_text_output` ❌ (RC-1)

### Category 3: Deduplication (5/8 ✅ — 3 failing)

- `integration::dedup_tests::dedup_atoms_not_duplicated` ✅
- `integration::dedup_tests::dedup_exact_match` ✅
- `integration::dedup_tests::dedup_graph_valid_after_inserts` ✅
- `integration::dedup_tests::dedup_no_duplicate_vertices` ✅
- `integration::dedup_tests::dedup_shared_prefix` ✅
- `integration::dedup_tests::dedup_insert_then_read` ❌ (RC-1)
- `integration::dedup_tests::dedup_multiple_reads_same_result` ❌ (RC-2)
- `integration::dedup_tests::dedup_read_sequence_finds_existing` ❌ (RC-2)

### Category 4: File Input (3/4 ✅ — 1 failing)

- `integration::file_input_tests::file_read_empty` ✅
- `integration::file_input_tests::file_read_nonexistent` ✅
- `integration::file_input_tests::file_read_unicode` ✅
- `integration::file_input_tests::file_read_basic` ❌ (RC-1)

### Category 5: REPL Integration

- ⏭️ Deferred — requires CLI binary process spawning and REPL smart-parsing changes

### Category 6: Edge Cases (5/6 ✅ — 1 failing)

- `integration::edge_case_tests::error_read_no_workspace` ✅
- `integration::edge_case_tests::error_read_invalid_index` ✅
- `integration::edge_case_tests::error_read_closed_workspace` ✅
- `integration::edge_case_tests::edge_single_char` ✅
- `integration::edge_case_tests::edge_two_chars` ✅
- `integration::edge_case_tests::edge_repeated_single_char` ❌ (RC-3)

### Category 0: Skill 3 Exploration — Green Today (8/8 ✅)

These tests document the **current** (broken) behaviour to prevent silent regressions while RC-1
is being fixed. They assert on what actually happens today, not what should happen:

- `integration::skill3_exploration::skill3_exp_d_atom_deduplication` ✅
- `integration::skill3_exploration::skill3_exp_h_graph_valid_after_completion_sequence` ✅
- `integration::skill3_exploration::skill3_exp_j_single_char_is_too_short` ✅
- `integration::skill3_exploration::skill3_obs1_multi_char_insert_reports_already_existed_true_today` ✅
- `integration::skill3_exploration::skill3_obs2_same_text_twice_same_atom_index_today` ✅
- `integration::skill3_exploration::skill3_obs3_atoms_correctly_auto_created_and_deduped` ✅
- `integration::skill3_exploration::skill3_obs4_cross_word_atom_sharing` ✅
- `integration::skill3_exploration::skill3_obs5_graph_valid_under_rc1_conditions` ✅

### Ngrams Oracle — Self-Check Machinery (4/4 ✅)

These test the graph comparison infrastructure independently of context-read behaviour. If any of
these fail, the oracle machinery itself is broken — not context-read.

- `integration::ngrams_oracle_tests::oracle_machinery_label_map_roundtrip` ✅
- `integration::ngrams_oracle_tests::oracle_machinery_self_check_ab` ✅
- `integration::ngrams_oracle_tests::oracle_machinery_self_check_abab` ✅
- `integration::ngrams_oracle_tests::oracle_machinery_self_check_abcabc` ✅

---

## Key Observations

1. **Atom management is stable** — all 6 atom tests and all atom-related dedup tests pass. The
   graph layer correctly creates, deduplicates, and looks up atoms.

2. **Read on fresh sequences works** — `read_sequence` on a workspace with no prior inserts
   correctly returns `width=1` for single chars (`edge_single_char`, `read_sequence_single_char`)
   and `width=N` for multi-char sequences (`read_sequence_text`, `edge_two_chars`). The read path
   itself is not broken — only the *after-insert* read path (RC-2) and the *insert output* (RC-1)
   are broken.

3. **Graph integrity holds** — `validate_graph` passes in every test that exercises it, including
   after multiple `insert_sequence` calls with the RC-1 bug present. The broken inserts do not
   corrupt the graph; they merely produce incomplete structure.

4. **Failures cluster around two patterns:**
   - **RC-1 (5 failures):** `insert_sequence` produces width=1 root. Fix = outer loop.
   - **RC-2 (3 failures):** `read_sequence` returns `None` after a prior insert. Fix = likely
     resolves after RC-1; may need additional `ReadCtx` investigation.
   - **RC-3 (1 failure):** Repeated single-char cursor bug. Fix = separate cursor redesign.

5. **`file_read_basic` is a special case of RC-1** — `ReadFile` uses `read_sequence` which works
   on a fresh graph, but without the outer accumulation loop, repeated characters are deduplicated
   prematurely ("helo wrd" ≠ "hello world"). This is RC-1 manifesting in the read path.

6. **22 ignored tests are gates:** Once RC-1 is fixed, 20 tests (`skill3_exp_*` + `oracle_*`)
   should immediately turn green. The two RC-3 tests require a separate fix.

---

## Fix Priority Order

| Priority | Root Cause | Expected Tests Unblocked | Effort |
|----------|-----------|--------------------------|--------|
| 🔴 P1 | RC-1: Add outer loop to `insert_sequence` | 5 failing → green; 20 ignored → green | Medium |
| 🟠 P2 | RC-2: Fix `ReadCtx::read_sequence` after insert | 3 failing → green | Unknown (re-evaluate after P1) |
| 🟡 P3 | RC-3: Cursor advancement for repeated chars | 1 failing → green; 2 ignored → green | High |

---

## Reproduction Commands

```sh
# Run all integration tests (shows pass/fail/ignore summary)
cargo test -p context-cli --test cli_integration

# Run only the failing tests
cargo test -p context-cli --test cli_integration read_known_pattern read_text_output read_produces_decomposition_tree read_sequence_after_insert dedup_insert_then_read dedup_read_sequence_finds_existing dedup_multiple_reads_same_result edge_repeated_single_char file_read_basic

# Run RC-1-gated ignored tests (after fix, all should pass)
cargo test -p context-cli --test cli_integration skill3_exp_ -- --include-ignored
cargo test -p context-cli --test cli_integration oracle_ -- --include-ignored

# Run with output for debugging
cargo test -p context-cli --test cli_integration -- --nocapture 2>&1 | grep -E "(FAILED|ok|ignored)"
```
