//! Skill 3 Exploration Tests: Context Completion — Finding Meaning in Small Tokens
//!
//! # Purpose
//!
//! This module contains integration tests written as **experiments** to observe
//! and validate the behaviour of `insert_next_match` through the public API.
//! The results of these experiments directly inform the dungeon-crawler skill
//! document `docs/skills/03_context_completion.md`.
//!
//! # What We Are Exploring
//!
//! `insert_next_match` — the core insertion primitive — can return three
//! outcomes:
//!
//! | Outcome        | Meaning                                              |
//! |----------------|------------------------------------------------------|
//! | `Created`      | A new compound token was built from existing pieces. |
//! | `Complete`     | The entire input already exists; nothing was added.  |
//! | `NoExpansion`  | The matched portion is the best we can do; the rest  |
//! |                | of the input must be handled in subsequent calls.    |
//!
//! The `InsertResult` returned by `Command::InsertSequence` surfaces these as:
//!   - `already_existed == false`  →  `Created`
//!   - `already_existed == true`   →  `Complete` or `NoExpansion`
//!
//! # Exploration Findings (recorded 2026-03-14)
//!
//! ## What We Expected
//!
//! Calling `insert_text("hello")` on a fresh workspace should:
//!   1. Auto-create atoms h, e, l, o (4 unique vertices)
//!   2. Call `insert_next_match([h, e, l, l, o])`
//!   3. The engine greedily finds the largest match starting at cursor 0,
//!      builds compound sub-tokens as needed, and ultimately wraps the whole
//!      sequence into a root token of width 5.
//!   4. Return `already_existed = false`, `token.width = 5`.
//!
//! ## What We Observed (RC-1)
//!
//! `insert_sequence` calls `insert_next_match` **only once**.  Because the
//! freshly-created atoms are all single-width and no compound tokens exist yet,
//! the very first call matches only the first atom ('h', width 1) and returns
//! `NoExpansion` — meaning "I found a complete token at position 0, but the
//! query extends beyond it; the caller is responsible for advancing the
//! cursor."
//!
//! Since `already_existed = !outcome.is_expanded()` and `NoExpansion` is not
//! `Created`, this evaluates to `already_existed = true` even on the very
//! first insert.  The root token returned has width 1 (the first atom), not
//! the expected width of the full input.
//!
//! **Root cause**: `insert_sequence` is missing the outer loop that repeatedly
//! calls `insert_next_match` until the entire input is consumed.  That outer
//! loop exists in `ReadCtx` (the read pipeline), but has not yet been wired
//! into the write-path `insert_sequence` in `context-api`.
//!
//! ## Impact on Tests in This Module
//!
//! Tests that check `already_existed == false` or `token.width == N` (where
//! N > 1) are tagged `#[ignore = "RC-1: ..."]`.  They represent the *intended*
//! behaviour; they will pass once the outer loop is added.
//!
//! Tests that work with what the engine *actually does today* (atom creation,
//! graph validity, single-char guard) run normally and are currently green.
//!
//! # Known Limitations
//!
//! | Code | Root Cause                                                   |
//! |------|--------------------------------------------------------------|
//! | RC-1 | `insert_sequence` calls `insert_next_match` once — only the  |
//! |      | first atom is matched; compound creation never happens.       |
//! | RC-2 | `read_sequence` returns `None` after a prior insert.          |
//! | RC-3 | Repeat/overlap handling broken for inputs like "aaaa".        |

use crate::common::helpers::*;

// ═══════════════════════════════════════════════════════════════════════════
//  SECTION 1 — Intended Behaviour Tests (currently #[ignore] due to RC-1)
//  These define the contract that insert_sequence WILL satisfy when fixed.
// ═══════════════════════════════════════════════════════════════════════════

// ── Experiment A ─────────────────────────────────────────────────────────────
//
// SCENARIO: Insert a sequence that has never been seen before.
//
// INTENDED OUTCOME: Created
//   - `already_existed == false`
//   - `token.width` equals the character count of the input
//
// SKILL-DOC MAPPING: "Inserting 'hello' When Nothing Exists — Outcome: Created"

#[test]
#[ignore = "RC-1: insert_sequence does not loop over insert_next_match; \
            always returns NoExpansion for first atom — FAILING_TESTS.md"]
fn skill3_exp_a_fresh_insert_is_created() {
    let mut ws = TestWorkspace::new("sk3-exp-a");

    let result = ws.insert_text("hello");
    let ir = unwrap_insert_result(&result);

    assert!(
        !ir.already_existed,
        "first insert of 'hello' must be Created (already_existed=false)"
    );
    assert_eq!(
        ir.token.width, 5,
        "width of 'hello' must equal its 5-character length"
    );
}

// ── Experiment B ─────────────────────────────────────────────────────────────
//
// SCENARIO: Insert the same sequence twice.
//
// INTENDED OUTCOME: second call → Complete
//   - `already_existed == true`
//   - Same `token.index` as the first call
//   - Same `token.width`
//
// SKILL-DOC MAPPING: "Inserting 'hello' When 'hello' Already Exists — Outcome: Complete"

#[test]
#[ignore = "RC-1: first insert is not Created, so token.index comparison is \
            meaningless and width is wrong — FAILING_TESTS.md"]
fn skill3_exp_b_second_insert_is_complete() {
    let mut ws = TestWorkspace::new("sk3-exp-b");

    let r1 = ws.insert_text("hello");
    let ir1 = unwrap_insert_result(&r1);

    let r2 = ws.insert_text("hello");
    let ir2 = unwrap_insert_result(&r2);

    assert!(
        !ir1.already_existed,
        "first insert must be Created (already_existed=false)"
    );
    assert!(
        ir2.already_existed,
        "second insert must be Complete (already_existed=true)"
    );
    assert_eq!(
        ir1.token.index, ir2.token.index,
        "both calls must return the same token index — same vertex in the graph"
    );
    assert_eq!(
        ir1.token.width, ir2.token.width,
        "width must be stable across repeated inserts"
    );
}

// ── Experiment C ─────────────────────────────────────────────────────────────
//
// SCENARIO: Insert two fragments ("hel", "lo") and then their concatenation
//           ("hello").
//
// INTENDED OUTCOME:
//   - "hel" → Created, width=3
//   - "lo"  → Created, width=2
//   - "hello" → Created (new compound token), width=5
//   - No duplicate atoms: h, e, l, o = 4 unique atoms
//
// SKILL-DOC MAPPING:
//   "Inserting 'hello' When 'hel' and 'lo' Exist — Outcome: Created"
//   The craftsman finds the existing parts on the workbench and joins them.

#[test]
#[ignore = "RC-1: insert_sequence does not produce compound tokens; \
            'hel' and 'lo' are never Created — FAILING_TESTS.md"]
fn skill3_exp_c_compound_from_known_pieces() {
    let mut ws = TestWorkspace::new("sk3-exp-c");

    let r_hel = ws.insert_text("hel");
    let ir_hel = unwrap_insert_result(&r_hel);
    assert!(!ir_hel.already_existed, "'hel' must be Created");
    assert_eq!(ir_hel.token.width, 3, "'hel' must have width 3");

    let r_lo = ws.insert_text("lo");
    let ir_lo = unwrap_insert_result(&r_lo);
    assert!(!ir_lo.already_existed, "'lo' must be Created");
    assert_eq!(ir_lo.token.width, 2, "'lo' must have width 2");

    let stats_before = ws.get_statistics();
    let s_before = unwrap_statistics(&stats_before);

    let r_hello = ws.insert_text("hello");
    let ir_hello = unwrap_insert_result(&r_hello);

    assert!(!ir_hello.already_existed, "'hello' must be Created");
    assert_eq!(ir_hello.token.width, 5, "'hello' must have width 5");

    let stats_after = ws.get_statistics();
    let s_after = unwrap_statistics(&stats_after);

    // No new atoms: h, e, l, o were already present
    assert_eq!(
        s_before.atom_count, s_after.atom_count,
        "inserting 'hello' after 'hel'+'lo' must not create new atoms"
    );

    // The graph must have grown (the "hello" token is new)
    assert!(
        s_after.vertex_count > s_before.vertex_count,
        "inserting 'hello' must add at least one new vertex to the graph"
    );
}

// ── Experiment E ─────────────────────────────────────────────────────────────
//
// SCENARIO: Insert "hello" then insert "helloworld".
//
// INTENDED OUTCOME for "helloworld":
//   - Created, width = 10
//   - Internally: NoExpansion for the "hello" prefix (already known),
//     then "world" is built from atoms and combined into one root token.
//
// SKILL-DOC MAPPING:
//   "Inserting 'helloworld' When 'hello' Exists But 'world' Doesn't"

#[test]
#[ignore = "RC-1: insert_sequence does not loop; 'hello' is never Created, \
            so testing 'helloworld' compound is meaningless — FAILING_TESTS.md"]
fn skill3_exp_e_known_prefix_then_new_suffix() {
    let mut ws = TestWorkspace::new("sk3-exp-e");

    let r_hello = ws.insert_text("hello");
    let ir_hello = unwrap_insert_result(&r_hello);
    assert!(!ir_hello.already_existed);
    assert_eq!(ir_hello.token.width, 5);

    let r_hw = ws.insert_text("helloworld");
    let ir_hw = unwrap_insert_result(&r_hw);

    assert!(!ir_hw.already_existed, "'helloworld' must be Created");
    assert_eq!(ir_hw.token.width, 10, "'helloworld' must have width 10");

    // "hello" token must be unchanged
    let r_hello2 = ws.insert_text("hello");
    let ir_hello2 = unwrap_insert_result(&r_hello2);
    assert!(
        ir_hello2.already_existed,
        "'hello' still exists after 'helloworld'"
    );
    assert_eq!(ir_hello.token.index, ir_hello2.token.index);
}

// ── Experiment F ─────────────────────────────────────────────────────────────
//
// SCENARIO: Insert "hello" and "world" individually, then insert "helloworld".
//
// INTENDED OUTCOME:
//   - "helloworld" → Created, width=10
//   - Unique atoms: h,e,l,o,w,r,d = 7

#[test]
#[ignore = "RC-1: neither 'hello' nor 'world' is Created by insert_sequence; \
            the compound test is therefore meaningless — FAILING_TESTS.md"]
fn skill3_exp_f_both_pieces_known_before_compound() {
    let mut ws = TestWorkspace::new("sk3-exp-f");

    let r_hello = ws.insert_text("hello");
    let ir_hello = unwrap_insert_result(&r_hello);
    assert!(!ir_hello.already_existed);
    assert_eq!(ir_hello.token.width, 5);

    let r_world = ws.insert_text("world");
    let ir_world = unwrap_insert_result(&r_world);
    assert!(!ir_world.already_existed);
    assert_eq!(ir_world.token.width, 5);

    let r_hw = ws.insert_text("helloworld");
    let ir_hw = unwrap_insert_result(&r_hw);

    assert!(!ir_hw.already_existed, "'helloworld' must be Created");
    assert_eq!(ir_hw.token.width, 10, "'helloworld' width must be 10");

    let stats = ws.get_statistics();
    let s = unwrap_statistics(&stats);
    // h,e,l,o,w,r,d = 7 unique chars
    assert_eq!(
        s.atom_count, 7,
        "expected 7 unique atom vertices (h,e,l,o,w,r,d), got {}",
        s.atom_count
    );
}

// ── Experiment G ─────────────────────────────────────────────────────────────
//
// SCENARIO: Insert same text 4× — all but the first should be Complete.

#[test]
#[ignore = "RC-1: first insert is not Created; idempotent test relies on a \
            correct initial Created outcome — FAILING_TESTS.md"]
fn skill3_exp_g_idempotent_multiple_complete_calls() {
    let mut ws = TestWorkspace::new("sk3-exp-g");

    let r1 = ws.insert_text("dungeon");
    let ir1 = unwrap_insert_result(&r1);
    assert!(!ir1.already_existed, "first insert must be Created");

    for i in 2u8..=4 {
        let r = ws.insert_text("dungeon");
        let ir = unwrap_insert_result(&r);
        assert!(
            ir.already_existed,
            "call #{i}: insert of 'dungeon' must be Complete"
        );
        assert_eq!(ir.token.index, ir1.token.index, "call #{i}: index stable");
        assert_eq!(ir.token.width, ir1.token.width, "call #{i}: width stable");
    }
}

// ── Experiment I ─────────────────────────────────────────────────────────────
//
// SCENARIO: Minimum-length insert (exactly 2 characters).

#[test]
#[ignore = "RC-1: even a 2-char insert returns NoExpansion for the first atom; \
            already_existed=true and width=1 — FAILING_TESTS.md"]
fn skill3_exp_i_minimum_length_insert() {
    let mut ws = TestWorkspace::new("sk3-exp-i");

    let result = ws.insert_text("ab");
    let ir = unwrap_insert_result(&result);

    assert!(!ir.already_existed, "two-char insert must be Created");
    assert_eq!(ir.token.width, 2, "two-char token must have width 2");
}

// ── Experiment K ─────────────────────────────────────────────────────────────
//
// SCENARIO: The dungeon analogy — build "dungeon run" and "dungeon dungeon".

#[test]
#[ignore = "RC-1: multi-char sequences are not Created; dungeon analogy \
            compound test requires a working insert loop — FAILING_TESTS.md"]
fn skill3_exp_k_dungeon_analogy_compound() {
    let mut ws = TestWorkspace::new("sk3-exp-k");

    let r1 = ws.insert_text("dungeon");
    let ir1 = unwrap_insert_result(&r1);
    assert!(!ir1.already_existed, "'dungeon' must be Created");
    assert_eq!(ir1.token.width, 7);

    let r2 = ws.insert_text("dungeon run");
    let ir2 = unwrap_insert_result(&r2);
    assert!(!ir2.already_existed, "'dungeon run' must be Created");
    assert_eq!(ir2.token.width, 11);

    let r3 = ws.insert_text("dungeon dungeon");
    let ir3 = unwrap_insert_result(&r3);
    assert!(!ir3.already_existed, "'dungeon dungeon' must be Created");
    assert_eq!(ir3.token.width, 15);

    let r4 = ws.insert_text("dungeon");
    let ir4 = unwrap_insert_result(&r4);
    assert!(ir4.already_existed, "second 'dungeon' must be Complete");
    assert_eq!(ir4.token.index, ir1.token.index);

    let r5 = ws.insert_text("dungeon dungeon");
    let ir5 = unwrap_insert_result(&r5);
    assert!(
        ir5.already_existed,
        "second 'dungeon dungeon' must be Complete"
    );
    assert_eq!(ir5.token.index, ir3.token.index);

    let report = ws.validate_graph();
    match report {
        context_api::commands::CommandResult::ValidationReport(r) => {
            assert!(r.valid, "graph must be valid; issues: {:?}", r.issues);
        },
        other => panic!("expected ValidationReport, got {other:?}"),
    }
}

// ── Experiment L ─────────────────────────────────────────────────────────────
//
// SCENARIO: Bulk Created→Complete cycle for 5 distinct 2-char sequences.

#[test]
#[ignore = "RC-1: 2-char inserts are not Created — FAILING_TESTS.md"]
fn skill3_exp_l_bulk_created_then_complete() {
    let mut ws = TestWorkspace::new("sk3-exp-l");

    let sequences = ["ab", "cd", "ef", "gh", "ij"];

    let original_indices: Vec<usize> = sequences
        .iter()
        .map(|text| {
            let r = ws.insert_text(text);
            let ir = unwrap_insert_result(&r);
            assert!(
                !ir.already_existed,
                "first insert of '{text}' must be Created"
            );
            ir.token.index
        })
        .collect();

    for (text, &original_index) in sequences.iter().zip(original_indices.iter())
    {
        let r = ws.insert_text(text);
        let ir = unwrap_insert_result(&r);
        assert!(ir.already_existed, "re-insert of '{text}' must be Complete");
        assert_eq!(
            ir.token.index, original_index,
            "re-insert of '{text}' must return the same token index"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  SECTION 2 — Currently-Green Tests
//  These test behaviour that works correctly today and must not regress.
// ═══════════════════════════════════════════════════════════════════════════

// ── Experiment D ─────────────────────────────────────────────────────────────
//
// SCENARIO: Verify atom deduplication — "hello" shares atoms with "hel"+"lo".
//
// OBSERVATION (verified GREEN 2026-03-14):
//   Even though insert_sequence does not yet produce compound tokens,
//   it DOES correctly auto-create atoms and deduplicate them.
//   Calling insert_text("hel"), insert_text("lo"), insert_text("hello")
//   results in exactly 4 atom vertices: h, e, l, o.
//   'l' appears twice in "hello" but maps to one atom vertex.
//
// SKILL-DOC MAPPING: "The graph grows with each insertion — future reads
//   benefit from previously created tokens"

#[test]
fn skill3_exp_d_atom_deduplication() {
    let mut ws = TestWorkspace::new("sk3-exp-d");

    ws.insert_text("hel");
    ws.insert_text("lo");
    ws.insert_text("hello");

    let stats = ws.get_statistics();
    let s = unwrap_statistics(&stats);

    // h, e, l, o — even though 'l' appears twice in "hello" it is one atom.
    assert_eq!(
        s.atom_count, 4,
        "expected exactly 4 unique atom vertices (h, e, l, o), got {}",
        s.atom_count
    );
}

// ── Experiment H ─────────────────────────────────────────────────────────────
//
// SCENARIO: Graph integrity is maintained after a series of insert calls.
//
// OBSERVATION (verified GREEN 2026-03-14):
//   Even with RC-1 present, validate_graph reports no issues.
//   The partial NoExpansion inserts leave the graph in a structurally
//   consistent (if semantically incomplete) state.
//
// SKILL-DOC MAPPING: Background health check — confirms the workshop leaves
//   the dungeon map consistent.

#[test]
fn skill3_exp_h_graph_valid_after_completion_sequence() {
    let mut ws = TestWorkspace::new("sk3-exp-h");

    // Build a small graph — each call inserts atoms and attempts a join
    ws.insert_text("ab");
    ws.insert_text("bc");
    ws.insert_text("abc");
    ws.insert_text("cd");
    ws.insert_text("bcd");
    // Repeated calls (exercise the Complete / NoExpansion paths)
    ws.insert_text("ab");
    ws.insert_text("abc");

    let report = ws.validate_graph();
    match report {
        context_api::commands::CommandResult::ValidationReport(r) => {
            assert!(
                r.valid,
                "graph must be valid after a series of insert cycles; \
                 issues: {:?}",
                r.issues
            );
        },
        other => panic!("expected ValidationReport, got {other:?}"),
    }
}

// ── Experiment J ─────────────────────────────────────────────────────────────
//
// SCENARIO: Single-character insert now succeeds (returns the atom token).
//
// UPDATED (RC-1 fix): insert_sequence now accepts single-character inputs and
//   returns the corresponding atom token directly. Only empty strings are
//   rejected with QueryTooShort. Use the `atom` REPL command as an alternative,
//   but the sequence API no longer rejects single chars.

#[test]
fn skill3_exp_j_single_char_is_too_short() {
    let mut ws = TestWorkspace::new("sk3-exp-j");

    // Single-char is now valid — returns the atom token with width=1.
    let result = ws.exec(context_api::commands::Command::InsertSequence {
        workspace: ws.name.clone(),
        text: "a".to_string(),
    });

    assert!(
        result.is_ok(),
        "inserting a single character should succeed and return the atom token"
    );

    // Empty string is still rejected.
    let empty_result =
        ws.exec(context_api::commands::Command::InsertSequence {
            workspace: ws.name.clone(),
            text: String::new(),
        });
    assert!(
        empty_result.is_err(),
        "inserting an empty string must return an error (QueryTooShort)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  SECTION 3 — Observation Tests (updated after RC-1 fix)
//  These tests now assert the CORRECT post-fix behaviour. The RC-1 bug
//  (every multi-char insert returning already_existed=true with the first
//  atom) has been resolved. Tests updated accordingly.
// ═══════════════════════════════════════════════════════════════════════════

// ── Observation 1 ────────────────────────────────────────────────────────────
//
// RC-1 FIXED: First insert of a multi-char sequence now correctly returns
//   already_existed=false and a composite token with the full width.

#[test]
fn skill3_obs1_multi_char_insert_reports_already_existed_true_today() {
    let mut ws = TestWorkspace::new("sk3-obs1");

    // RC-1 fixed: first insert creates the composite token.
    let r = ws.insert_text("hello");
    let ir = unwrap_insert_result(&r);

    assert!(
        !ir.already_existed,
        "[RC-1 fixed] first multi-char insert should return already_existed=false"
    );

    // The width returned is 5 (the full composite token), not 1.
    assert_eq!(
        ir.token.width, 5,
        "[RC-1 fixed] width should be 5 (full composite token)"
    );
}

// ── Observation 2 ────────────────────────────────────────────────────────────
//
// RC-1 FIXED: Two calls to insert_text with the same text now correctly
//   return the same composite token index. The first call creates the token
//   (already_existed=false), the second finds it (already_existed=true).

#[test]
fn skill3_obs2_same_text_twice_same_atom_index_today() {
    let mut ws = TestWorkspace::new("sk3-obs2");

    let r1 = ws.insert_text("hello");
    let r2 = ws.insert_text("hello");

    let ir1 = unwrap_insert_result(&r1);
    let ir2 = unwrap_insert_result(&r2);

    // Both calls return the same index (same composite token).
    assert_eq!(
        ir1.token.index, ir2.token.index,
        "[RC-1 fixed] same text inserted twice should return the same token index"
    );

    // First call creates the token, second finds it.
    assert!(
        !ir1.already_existed,
        "[RC-1 fixed] first call should return already_existed=false"
    );
    assert!(
        ir2.already_existed,
        "[RC-1 fixed] second call should return already_existed=true"
    );
}

// ── Observation 3 ────────────────────────────────────────────────────────────
//
// WHAT WE OBSERVE: Atoms ARE correctly deduplicated today.
//   Inserting "hel", "lo", "hello" produces exactly 4 atom vertices.
//   This part of the engine works correctly; only the outer loop that
//   builds compound tokens is missing.

#[test]
fn skill3_obs3_atoms_correctly_auto_created_and_deduped() {
    let mut ws = TestWorkspace::new("sk3-obs3");

    // Each call auto-creates new atoms and reuses existing ones
    ws.insert_text("hel"); // creates h(0), e(1), l(2)
    ws.insert_text("lo"); // reuses l(2), creates o(3)

    let after_two = ws.get_statistics();
    let s2 = unwrap_statistics(&after_two);
    assert_eq!(
        s2.atom_count, 4,
        "after 'hel'+'lo': exactly 4 atoms (h,e,l,o), got {}",
        s2.atom_count
    );

    ws.insert_text("hello"); // reuses all 4 atoms, no new ones

    let after_three = ws.get_statistics();
    let s3 = unwrap_statistics(&after_three);
    assert_eq!(
        s3.atom_count, 4,
        "after adding 'hello': still exactly 4 atoms, got {}",
        s3.atom_count
    );
}

// ── Observation 4 ────────────────────────────────────────────────────────────
//
// WHAT WE OBSERVE: Different texts that share characters produce the correct
//   atom deduplication count.
//   "hello" (h,e,l,o) + "world" (w,o,r,l,d) → 7 unique atoms h,e,l,o,w,r,d.

#[test]
fn skill3_obs4_cross_word_atom_sharing() {
    let mut ws = TestWorkspace::new("sk3-obs4");

    ws.insert_text("hello"); // h,e,l,o  → 4 atoms
    ws.insert_text("world"); // w,o,r,l,d → adds w,r,d; reuses o,l → 7 atoms total

    let stats = ws.get_statistics();
    let s = unwrap_statistics(&stats);

    // Unique chars across "hello" + "world": h, e, l, o, w, r, d = 7
    assert_eq!(
        s.atom_count, 7,
        "expected 7 unique atom vertices across 'hello'+'world', got {}",
        s.atom_count
    );
}

// ── Observation 5 ────────────────────────────────────────────────────────────
//
// WHAT WE OBSERVE: Graph structure remains valid even under RC-1 conditions.
//   Many inserts, all returning NoExpansion, still leave the graph consistent.

#[test]
fn skill3_obs5_graph_valid_under_rc1_conditions() {
    let mut ws = TestWorkspace::new("sk3-obs5");

    // Insert a variety of sequences — all will hit NoExpansion today
    let texts = [
        "ab",
        "bc",
        "cd",
        "abc",
        "bcd",
        "abcd",
        "hello",
        "world",
        "helloworld",
    ];
    for text in texts {
        ws.insert_text(text);
    }

    let report = ws.validate_graph();
    match report {
        context_api::commands::CommandResult::ValidationReport(r) => {
            assert!(
                r.valid,
                "graph must be structurally valid even under RC-1; issues: {:?}",
                r.issues
            );
        },
        other => panic!("expected ValidationReport, got {other:?}"),
    }
}

// ── Experiment M (known-failing, RC-3) ───────────────────────────────────────
//
// SCENARIO: Insert a sequence where all characters are the same ("aaaa").
//
// STATUS: KNOWN FAILING — RC-3 (repeat/overlap handling broken).
//   See FAILING_TESTS.md for details.
//
// INTENDED OUTCOME (when fixed):
//   - Created, width=4, already_existed=false
//   - Atoms: a = 1 unique atom vertex

#[test]
#[ignore = "RC-3: repeat/overlap cursor advancement bug — see FAILING_TESTS.md"]
fn skill3_exp_m_repeated_char_known_failing() {
    let mut ws = TestWorkspace::new("sk3-exp-m");

    let result = ws.insert_text("aaaa");
    let ir = unwrap_insert_result(&result);

    assert!(!ir.already_existed);
    assert_eq!(ir.token.width, 4);

    let stats = ws.get_statistics();
    let s = unwrap_statistics(&stats);
    assert_eq!(s.atom_count, 1, "only atom 'a' should exist");
}
