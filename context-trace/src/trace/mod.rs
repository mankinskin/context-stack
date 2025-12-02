pub mod cache;
pub mod child;
pub mod has_graph;
pub mod state;
pub mod traceable;

use crate::{
    path::accessors::role::PathRole,
    trace::{
        cache::{
            TraceCache,
            key::directed::DirectedKey,
        },
        traceable::{
            TraceCommand,
            Traceable,
            role::TraceDirection,
        },
    },
};
use cache::key::directed::HasAtomPosition;
use has_graph::HasGraph;
use std::fmt::Debug;

#[derive(Debug)]
pub struct TraceCtx<G: HasGraph> {
    pub trav: G,
    pub cache: TraceCache,
}
impl<G: HasGraph> TraceCtx<G> {
    pub fn trace_command(
        &mut self,
        command: TraceCommand,
    ) {
        command.trace(self)
    }
}

pub trait TraceKey:
    HasAtomPosition + Debug + Clone + Copy + Into<DirectedKey>
{
}
impl<T: HasAtomPosition + Debug + Clone + Into<DirectedKey> + Copy> TraceKey
    for T
{
}

pub(crate) type RoleTraceKey<Role> =
    <<Role as PathRole>::Direction as TraceDirection>::Key;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BottomUp;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopDown;
