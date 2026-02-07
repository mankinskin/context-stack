use crate::{
    bands::{
        HasTokenRoleIters,
        PostfixIterator,
    },
    expansion::{
        chain::{
            link::{
                BandCap,
                BandExpansion,
            },
            ChainOp,
        },
        ExpansionCtx,
    },
};
use context_insert::*;
use context_trace::*;
use tracing::debug;

#[derive(Debug)]
pub struct ExpandCtx<'a> {
    pub ctx: &'a ExpansionCtx<'a>,
    pub postfix_path: IndexEndPath,
    pub postfix_iter: PostfixIterator<'a, HypergraphRef>,
}
impl<'a> ExpandCtx<'a> {
    pub fn try_new(ctx: &'a ExpansionCtx<'a>) -> Option<Self> {
        let last_end = ctx.last().last_token();
        debug!(last_end_postfix = ?last_end, "Try new ExpandCtx");
        let mut postfix_iter = last_end.postfix_iter(ctx.graph.clone());
        if let Some((postfix_location, _)) = postfix_iter.next() {
            debug!(initial_postfix_location = ?postfix_location, "ExpandCtx created");
            Some(Self {
                ctx,
                postfix_path: IndexEndPath::from(postfix_location),
                postfix_iter,
            })
        } else {
            debug!("ExpandCtx: no postfix positions available");
            None
        }
    }
}
impl Iterator for ExpandCtx<'_> {
    type Item = ChainOp;
    fn next(&mut self) -> Option<Self::Item> {
        self.postfix_iter.next().map(|(postfix_location, postfix)| {
            let last_end_bound = self.ctx.last().end_bound;
            let start_bound = *last_end_bound - *postfix.width();
            self.postfix_path.path_append(postfix_location);
            let result = match ToInsertCtx::<IndexWithPath>::insert(
                &self.ctx.graph,
                self.ctx.cursor.cursor.clone(),
            ) {
                Ok(expansion) => {
                    debug!(
                        postfix_location = ?postfix_location,
                        postfix = ?postfix,
                        start_bound = ?start_bound,
                        expansion_index = ?expansion.index,
                        "ExpandCtx::next -> Expansion"
                    );
                    ChainOp::Expansion(BandExpansion {
                        start_bound: start_bound.into(),
                        expansion,
                        postfix_path: self.postfix_path.clone(),
                    })
                },
                Err(_) => {
                    debug!(
                        postfix_location = ?postfix_location,
                        postfix = ?postfix,
                        start_bound = ?start_bound,
                        "ExpandCtx::next -> Cap"
                    );
                    ChainOp::Cap(BandCap {
                        postfix_path: self.postfix_path.clone(),
                        expansion: postfix,
                        start_bound: start_bound.into(),
                    })
                },
            };
            result
        })
    }
}
