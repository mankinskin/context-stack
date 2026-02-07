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
            },
        },
        cursor::CursorCtx,
        link::ExpansionLink,
    },
};
use chain::BandChain;

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
    pub(crate) chain: BandChain,
}
impl Iterator for ExpansionCtx<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        ExpandCtx::try_new(self)
            .and_then(|mut ctx| {
                // Find the next expansion or cap that can be applied at the current cursor position.
                ctx.find_map(|op| match &op {
                    ChainOp::Expansion(_) => Some(op),
                    ChainOp::Cap(cap) =>
                        self.chain.ends_at(cap.start_bound).map(|_| op),
                })
            })
            .and_then(|op| self.apply_op(op))
    }
}
impl<'a> ExpansionCtx<'a> {
    pub(crate) fn new(
        graph: HypergraphRef,
        cursor: &'a mut PatternRangePath,
    ) -> Self {
        debug!(cursor_root = ?cursor.path_root(), "New ExpansionCtx");
        let IndexWithPath { index: first, path } =
            match graph.insert_or_get_complete(cursor.clone()) {
                Ok(Ok(root)) => root,
                Ok(Err(root)) => root,
                Err(ErrorReason::SingleIndex(c)) => *c,
                Err(_) => {
                    // Get the first token from cursor's root pattern
                    let first = cursor.path_root()[0];
                    IndexWithPath {
                        index: first,
                        path: cursor.clone(),
                    }
                },
            };
        debug!(first_index = ?first, path_root = ?path.path_root(), "ExpansionCtx initialized");
        *cursor = path;

        Self {
            chain: BandChain::new(first),
            cursor: CursorCtx::new(graph, cursor),
        }
    }
    pub(crate) fn last(&self) -> &Band {
        self.chain.last().unwrap().band
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

                // handle case where expansion can be inserted after stack head (first band in current stack)
                let link = self.create_expansion_link(&exp);
                let complement =
                    ComplementBuilder::new(link).build(&self.cursor.graph);
                // TODO: Change this to a stack (list of overlaps with back contexts)
                self.chain
                    .append_front_complement(complement, exp.expansion.index);

                Some(exp.expansion.index)
            },
            ChainOp::Cap(cap) => {
                debug!(
                    cap_expansion = ?cap.expansion,
                    start_bound = ?cap.start_bound,
                    postfix_path = ?cap.postfix_path,
                    "apply_cap"
                );
                let mut first = self.chain.bands.pop_first().unwrap();
                first.append(cap.expansion);
                self.chain.append(first);
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
}
