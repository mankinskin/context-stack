pub(crate) mod block;
pub(crate) mod chain;
pub(crate) mod cursor;
pub(crate) mod link;
pub(crate) mod stack;

use crate::{
    complement::ComplementBuilder,
    expansion::{
        chain::{
            band::Band,
            expand::ExpandCtx,
            link::{
                BandExpansion,
                ChainOp,
                OverlapLink,
            },
            BandState,
        },
        cursor::CursorCtx,
        link::ExpansionLink,
    },
};

use context_insert::*;
use context_trace::*;

use derive_more::{
    Deref,
    DerefMut,
};
use tracing::debug;

#[derive(Debug, Deref, DerefMut)]
pub(crate) struct ExpansionCtx<'a> {
    #[deref]
    #[deref_mut]
    cursor: CursorCtx<'a>,
    pub(crate) state: BandState,
}
impl Iterator for ExpansionCtx<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        // If we already have an overlap, signal completion (must commit first)
        if self.state.has_overlap() {
            return None;
        }

        // First try to find overlaps via postfix expansion
        let overlap_result = ExpandCtx::try_new(self)
            .and_then(|mut ctx| {
                // Find the next expansion or cap that can be applied at the current cursor position.
                ctx.find_map(|op| match &op {
                    ChainOp::Expansion(_) => Some(op),
                    ChainOp::Cap(cap) => {
                        // Check if cap's start_bound matches state's end_bound
                        if cap.start_bound == self.state.end_bound() {
                            Some(op)
                        } else {
                            None
                        }
                    }
                })
            })
            .and_then(|op| self.apply_op(op));

        if overlap_result.is_some() {
            return overlap_result;
        }

        // No overlap found. 
        // If we had an external anchor (from existing root), overlap detection failed.
        // Use insert_or_get_complete to find longest prefix match from cursor.
        if self.state.has_external_anchor() {
            debug!("External anchor overlap detection failed, using insert_or_get_complete");
            
            // Clear the external anchor
            self.state.clear_external_anchor();
            
            // Find longest prefix match from cursor's current position
            let result: Result<Result<IndexWithPath, _>, _> = 
                self.cursor.graph.insert_or_get_complete(self.cursor.cursor.clone());
            
            let IndexWithPath { index: first, path } = match result {
                Ok(Ok(found)) => found,
                Ok(Err(found)) => found,
                Err(ErrorReason::SingleIndex(c)) => *c,
                Err(_) => {
                    // No match at all - use first cursor token
                    let first = self.cursor.cursor.path_root()[0];
                    debug!(first_cursor_token = ?first, "Cursor fallback: using first cursor token");
                    self.state.append(first);
                    return Some(first);
                }
            };
            
            debug!(first_token = ?first, "insert_or_get_complete returned");
            
            // Update cursor to the advanced position
            *self.cursor.cursor = path;
            
            // Set this as the first token in the band
            self.state.append(first);
            return Some(first);
        }

        // No external anchor - check if we've consumed all atoms yet.
        let atoms_consumed = *self.state.end_bound();
        let original_pattern = self.cursor.cursor.path_root();
        let total_atoms: usize = original_pattern.iter().map(|t| *t.width()).sum();
        
        if atoms_consumed < total_atoms {
            // There are still atoms to process. Find the remaining pattern elements.
            let mut acc_atoms = 0;
            let mut remaining_start_idx = 0;
            for (i, token) in original_pattern.iter().enumerate() {
                if acc_atoms >= atoms_consumed {
                    remaining_start_idx = i;
                    break;
                }
                acc_atoms += *token.width();
            }
            
            if remaining_start_idx >= original_pattern.len() {
                // All elements consumed
                return None;
            }
            
            // Create a pattern from the remaining elements
            let remaining: Pattern = original_pattern[remaining_start_idx..].to_vec().into();
            let remaining_path = PatternRangePath::from(remaining.clone());
            
            // Search for the next block
            let result: Result<Result<IndexWithPath, _>, _> = 
                self.cursor.graph.insert_or_get_complete(remaining_path);
            let next_block = match result {
                Ok(Ok(root)) => root.index,
                Ok(Err(root)) => root.index,
                Err(ErrorReason::SingleIndex(c)) => c.index,
                Err(_) => return None,
            };

            debug!(
                next_block = ?next_block,
                remaining = ?remaining,
                atoms_consumed = atoms_consumed,
                "Found next sequential block"
            );

            // Append token to the primary band
            self.state.append(next_block);

            return Some(next_block);
        }

        None
    }
}
impl<'a> ExpansionCtx<'a> {
    /// Create a new ExpansionCtx.
    /// 
    /// If `root_last_token` is provided, it will be used as the starting point
    /// for overlap detection. This allows finding overlaps between the existing
    /// root and the new cursor pattern.
    /// 
    /// If no root token is provided, the first token is created from the cursor pattern.
    pub(crate) fn new(
        graph: HypergraphRef,
        cursor: &'a mut PatternRangePath,
        root_last_token: Option<Token>,
    ) -> Self {
        debug!(
            cursor_root = ?cursor.path_root(),
            root_last_token = ?root_last_token,
            "New ExpansionCtx"
        );
        
        // If we have a root token, use it as the start for overlap detection
        if let Some(start_token) = root_last_token {
            debug!(start_token = ?start_token, "Using root's last token for overlap detection");
            return Self {
                // Use external anchor - tracks postfix iteration but no cursor atoms consumed
                state: BandState::with_external_anchor(start_token),
                cursor: CursorCtx::new(graph, cursor),
            };
        }
        
        // No root - use insert_or_get_complete to find longest prefix match
        let result: Result<Result<IndexWithPath, _>, _> = 
            graph.insert_or_get_complete(cursor.clone());
        
        let IndexWithPath { index: first, path } = match result {
            Ok(Ok(found)) => found,
            Ok(Err(found)) => found,
            Err(ErrorReason::SingleIndex(c)) => *c,
            Err(_) => {
                // No match - use first cursor token
                let first = cursor.path_root()[0];
                debug!(first_index = ?first, "No match, using first cursor token");
                return Self {
                    state: BandState::new(first),
                    cursor: CursorCtx::new(graph, cursor),
                };
            }
        };
        
        debug!(first_index = ?first, "ExpansionCtx initialized with insert_or_get_complete result");
        
        // Update cursor to the advanced position
        *cursor = path;

        Self {
            state: BandState::new(first),
            cursor: CursorCtx::new(graph, cursor),
        }
    }
    pub(crate) fn last(&self) -> &Band {
        self.state.primary()
    }
    pub(crate) fn apply_op(
        &mut self,
        op: ChainOp,
    ) -> Option<<Self as Iterator>::Item> {
        match op {
            ChainOp::Expansion(exp) => {
                debug!(
                    expansion_index = ?exp.expansion.index,
                    start_bound = ?exp.start_bound,
                    postfix_path = ?exp.postfix_path,
                    "apply_expansion"
                );
                *self.cursor.cursor = exp.expansion.path.clone();

                // Create expansion link with paths representing the overlap
                let expansion_link = self.create_expansion_link(&exp);
                
                // Create overlap link for the band state
                let overlap_link = self.create_overlap_link(&expansion_link);
                
                let complement =
                    ComplementBuilder::new(expansion_link).build(&self.cursor.graph);
                
                // Create overlap band [complement, expansion]
                let overlap_band = Band::from((
                    0.into(),
                    Pattern::from(vec![complement, exp.expansion.index]),
                ));
                
                // Transition to WithOverlap state
                let old_state = std::mem::take(&mut self.state);
                self.state = old_state.set_overlap(overlap_band, overlap_link);

                Some(exp.expansion.index)
            },
            ChainOp::Cap(cap) => {
                debug!(
                    cap_expansion = ?cap.expansion,
                    start_bound = ?cap.start_bound,
                    postfix_path = ?cap.postfix_path,
                    "apply_cap"
                );
                // Append to the primary band
                self.state.append(cap.expansion);
                None
            },
        }
    }
    fn create_expansion_link(
        &self,
        exp: &BandExpansion,
    ) -> ExpansionLink {
        debug!(
            expansion_index = ?exp.expansion.index,
            start_bound = ?exp.start_bound,
            "create_expansion_link"
        );
        let BandExpansion {
            postfix_path,
            expansion:
                IndexWithPath {
                    index: expansion, ..
                },
            start_bound,
        } = exp;
        let start_bound = (*start_bound).into();
        let overlap =
            postfix_path.role_leaf_token::<End, _>(&self.cursor.graph);
        use crate::bands::HasTokenRoleIters;
        let prefix_path = expansion
            .prefix_path(&self.cursor.graph, overlap.expect("overlap token"));

        ExpansionLink {
            start_bound,
            root_postfix: postfix_path.clone(),
            expansion_prefix: prefix_path,
        }
    }
    
    /// Create an overlap link from an expansion link.
    /// 
    /// The overlap link contains:
    /// - child_path: top-down path from starting root to expandable postfix (overlap region)
    /// - search_path: bottom-up then top-down path from expansion (overlap region from expansion's view)
    /// - start_bound: position where the overlap starts
    fn create_overlap_link(&self, expansion_link: &ExpansionLink) -> OverlapLink {
        debug!(
            root_postfix = ?expansion_link.root_postfix,
            expansion_prefix = ?expansion_link.expansion_prefix,
            start_bound = ?expansion_link.start_bound,
            "create_overlap_link"
        );
        
        OverlapLink {
            child_path: expansion_link.root_postfix.clone(),
            search_path: expansion_link.expansion_prefix.clone(),
            start_bound: expansion_link.start_bound,
        }
    }
}
