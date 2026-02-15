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
pub(crate) struct ExpansionCtx {
    #[deref]
    #[deref_mut]
    cursor: CursorCtx,
    pub(crate) state: BandState,
}
impl Iterator for ExpansionCtx {
    type Item = BandState;

    fn next(&mut self) -> Option<Self::Item> {
        // If we already have an overlap, signal completion (must commit first)
        if self.state.has_overlap() {
            return None;
        }

        ExpandCtx::try_new(self)
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
            .and_then(|op| self.apply_op(op))
    }
}
impl ExpansionCtx {
    /// Create a new ExpansionCtx.
    /// 
    /// If `root_last_token` is provided, it will be used as the starting point
    /// for overlap detection. This allows finding overlaps between the existing
    /// root and the new cursor pattern.
    /// 
    /// If no root token is provided, the first token is created from the cursor pattern.
    pub(crate) fn new(
        graph: HypergraphRef,
        cursor: PatternRangePath,
        band: Option<BandState>,
    ) -> Self {
        debug!(
            cursor_root = ?cursor.path_root(),
            //root_last_token = ?root_last_token,
            "New ExpansionCtx"
        );
        
        // If we have a root token, use it as the start for overlap detection
        if let Some(band) = band {
            debug!(band = ?band, "Using root's last token for overlap detection");
            Self {
                state: band,
                cursor: CursorCtx::new(graph, cursor),
            }
        } else {
        
            // No root - use insert_or_get_complete to find longest prefix match
            let result: Result<Result<IndexWithPath, _>, _> = 
                graph.insert_or_get_complete(cursor.clone());
            
            let IndexWithPath { index: first, path: cursor } = match result {
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

            Self {
                state: BandState::new(first),
                cursor: CursorCtx::new(graph, cursor),
            }
        }
    }
    //pub(crate) fn last(&self) -> &Band {
    //    self.state.primary()
    //}
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
                self.cursor.cursor = exp.expansion.path.clone();

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
                let state = BandState::default().set_overlap(overlap_band, overlap_link);

                Some(state)
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
