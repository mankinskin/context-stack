//! Acceptance tests of the context-api v2 contract (plan
//! `agents/plans/20261126_PLAN_CONTEXT_TOOLS/01_CORE_API.md` §1.4–§1.8).
//!
//! Written against the public contract only (`Session::execute`,
//! `Request`/`Command`/`Response`, `types`), as an independent oracle for
//! the implementation. Expected results come from brute force over the
//! input texts, the `ngrams` oracle and the reader fixtures.

mod common;
mod corpora;

mod contexts;
mod lifecycle;
mod oracle;
mod render;
mod roundtrip;
mod search;
mod serde_json;
mod validate;
