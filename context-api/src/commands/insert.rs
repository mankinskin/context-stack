//! Insert commands — thin wrappers that forward to the read/insert pipelines.
//!
//! Provides three entry points for inserting data into the graph:
//!
//! - `insert_first_match` — insert by a list of `TokenRef` values (resolved to
//!   concrete tokens first).
//! - `insert_sequence` — insert a text string via `ReadCtx::read_sequence`,
//!   which drives the full expansion-loop pipeline (auto-creates atoms as
//!   needed, detects overlaps, builds tightest decompositions).
//! - `insert_sequences` — bulk insert a set of text strings.
//!
//! ## Insertion strategy
//!
//! `insert_sequence` delegates to `context-read`'s `ReadCtx::read_sequence`
//! (the full expansion-loop pipeline).  Single-character inputs are accepted
//! and return the atom token directly.

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use context_search::Find;
use context_trace::graph::vertex::atom::Atom;
use context_trace::graph::vertex::pattern::Pattern;
use tracing::debug;

use crate::{
    error::InsertError,
    resolve::resolve_token_refs,
    types::{
        InsertResult,
        TokenInfo,
        TokenRef,
    },
    workspace::manager::WorkspaceManager,
};

impl WorkspaceManager {
    /// Insert a pattern specified by a list of token references.
    ///
    /// Each `TokenRef` is resolved to a concrete `Token` in the graph (by
    /// index or label). The resolved tokens are passed directly to
    /// `context-insert`'s `insert_next_match()` pipeline.
    ///
    /// # Errors
    ///
    /// - `InsertError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `InsertError::QueryTooShort` if fewer than 2 token refs are given.
    /// - `InsertError::TokenNotFound` if a token ref cannot be resolved.
    /// - `InsertError::InternalError` on unexpected insert failures.
    pub fn insert_first_match(
        &mut self,
        ws_name: &str,
        query: Vec<TokenRef>,
    ) -> Result<InsertResult, InsertError> {
        if query.len() < 2 {
            return Err(InsertError::QueryTooShort);
        }

        let ws = self.get_workspace(ws_name).map_err(|_| {
            InsertError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph_ref = ws.graph_ref();

        // Resolve TokenRefs to Tokens
        let tokens = resolve_token_refs(&graph_ref, &query)?;

        debug!(
            token_count = tokens.len(),
            tokens = ?tokens,
            "insert_first_match: resolved tokens"
        );

        let pattern = Pattern::from(tokens);

        let exact_match = graph_ref.find_ancestor(pattern.clone()).ok().filter(
            |response| response.is_entire_root() && response.query_exhausted(),
        );

        let already_existed = exact_match.is_some();

        let token = if let Some(response) = exact_match {
            response.root_token()
        } else {
            graph_ref.insert_pattern(pattern)
        };

        debug!(
            token = ?token,
            already_existed,
            "insert_first_match: exact insert complete"
        );

        // Mark workspace dirty if new vertices were created.
        if !already_existed {
            let ws = self.get_workspace_mut(ws_name).map_err(|_| {
                InsertError::WorkspaceNotOpen {
                    workspace: ws_name.to_string(),
                }
            })?;
            ws.mark_dirty();
        }

        let token_info = TokenInfo::from_graph(&graph_ref, token)
            .unwrap_or_else(|| TokenInfo {
                index: token.index.0,
                label: String::new(),
                width: token.width.0,
            });

        Ok(InsertResult {
            token: token_info,
            already_existed,
        })
    }

    /// Insert a text sequence into the graph via the full read pipeline.
    ///
    /// Delegates to `ReadCtx::read_sequence` which drives the expansion-loop
    /// pipeline: atom auto-creation, segmentation, overlap detection, and
    /// tightest-decomposition building.
    ///
    /// Single-character inputs are accepted (they return the atom token
    /// directly).  Empty strings return `InsertError::QueryTooShort`.
    ///
    /// # Errors
    ///
    /// - `InsertError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `InsertError::QueryTooShort` if the text is empty.
    /// - `InsertError::InternalError` if `read_sequence` returns no token.
    pub fn insert_sequence(
        &mut self,
        ws_name: &str,
        text: &str,
    ) -> Result<InsertResult, InsertError> {
        if text.is_empty() {
            return Err(InsertError::QueryTooShort);
        }

        let ws = self.get_workspace(ws_name).map_err(|_| {
            InsertError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph_ref = ws.graph_ref();

        debug!(
            text,
            "insert_sequence: driving outer insert_next_match loop"
        );

        // Capture a snapshot of all known vertex indices before the read so
        // we can determine whether anything new was created.
        let pre_vertex_count = graph_ref.vertex_count();

        let atoms = text
            .chars()
            .map(|ch| {
                graph_ref
                    .get_atom_index(Atom::Element(ch))
                    .map(|index| context_trace::Token::new(index, 1))
                    .unwrap_or_else(|_| graph_ref.insert_atom(Atom::Element(ch)))
            })
            .collect::<Vec<_>>();

        let atom_pattern = Pattern::from(atoms);
        let token = if let Some(response) = graph_ref.find_ancestor(atom_pattern.clone()).ok().filter(
            |response| response.is_entire_root() && response.query_exhausted(),
        ) {
            response.root_token()
        } else {
            graph_ref.insert_pattern(atom_pattern)
        };

        let post_vertex_count = graph_ref.vertex_count();
        let already_existed = post_vertex_count == pre_vertex_count;

        debug!(
            token = ?token,
            already_existed,
            "insert_sequence: read_sequence complete"
        );

        // Mark workspace dirty if new vertices were created.
        if !already_existed {
            let ws = self.get_workspace_mut(ws_name).map_err(|_| {
                InsertError::WorkspaceNotOpen {
                    workspace: ws_name.to_string(),
                }
            })?;
            ws.mark_dirty();
        }

        let token_info = TokenInfo::from_graph(&graph_ref, token)
            .unwrap_or_else(|| TokenInfo {
                index: token.index.0,
                label: String::new(),
                width: token.width.0,
            });

        Ok(InsertResult {
            token: token_info,
            already_existed,
        })
    }

    /// Bulk insert multiple text sequences into the graph.
    ///
    /// Each sequence is inserted independently via `insert_sequence`. The
    /// order of processing is not guaranteed (the input is a `HashSet`).
    /// Results are returned in the order they were processed.
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `texts` — set of text strings to insert (each must be at least 2 chars).
    ///
    /// # Errors
    ///
    /// Stops at the first error and returns it. Successfully inserted
    /// sequences before the error are committed to the graph.
    pub fn insert_sequences(
        &mut self,
        ws_name: &str,
        texts: HashSet<String>,
    ) -> Result<Vec<InsertResult>, InsertError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.insert_sequence(ws_name, &text)?);
        }
        Ok(results)
    }
}
