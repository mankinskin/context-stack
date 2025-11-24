#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum EndReason {
    QueryExhausted,
    Mismatch,
    ChildExhausted,
}

impl std::fmt::Display for EndReason {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        match self {
            EndReason::QueryExhausted => write!(f, "QueryExhausted"),
            EndReason::Mismatch => write!(f, "Mismatch"),
            EndReason::ChildExhausted => write!(f, "ChildExhausted"),
        }
    }
}

// End types:
// - top down match-mismatch
// - top down match-query end
// - bottom up-no matching parents

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndState {
    pub(crate) reason: EndReason,
    pub(crate) path: PathCoverage,
    pub(crate) cursor: PatternCursor,
}

impl Traceable for &EndState {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        match &self.path {
            PathCoverage::Range(p) => p.trace(ctx),
            PathCoverage::Prefix(p) => p.trace(ctx),
            PathCoverage::Postfix(p) => p.trace(ctx),
            _ => {},
        }
    }
}

impl EndState {
    pub(crate) fn with_reason<G: HasGraph>(
        trav: G,
        reason: EndReason,
        parent: ParentCompareState,
    ) -> Self {
        let root_pos = *context_trace::path::accessors::path_accessor::StatePosition::root_pos(&parent.parent_state);
        Self {
            reason,
            path: PathCoverage::from_start_path(
                parent.parent_state.path,
                root_pos,
                &trav,
            ),
            cursor: parent.cursor,
        }
    }

    pub(crate) fn mismatch<G: HasGraph>(
        trav: G,
        parent: ParentCompareState,
    ) -> Self {
        Self::with_reason(trav, EndReason::Mismatch, parent)
    }
}

impl RootKey for EndState {
    fn root_key(&self) -> UpKey {
        UpKey::new(
            match &self.path {
                PathCoverage::Range(s) => s.path.root_parent(),
                PathCoverage::Postfix(p) => p.path.root_parent(),
                PathCoverage::Prefix(p) => p.path.root_parent(),
                PathCoverage::EntireRoot(c) => c.root_parent(),
            },
            match &self.path {
                PathCoverage::Range(s) => s.root_pos.into(),
                PathCoverage::Postfix(p) => p.root_pos.into(),
                PathCoverage::Prefix(_) => 0.into(),
                PathCoverage::EntireRoot(_) => 0.into(),
            },
        )
    }
}

impl_root! { GraphRoot for EndState, self =>
    match &self.path {
        PathCoverage::EntireRoot(c) => c.root_parent(),
        PathCoverage::Range(p) => p.path.root_parent(),
        PathCoverage::Postfix(p) => p.path.root_parent(),
        PathCoverage::Prefix(p) => p.path.root_parent(),
    }
}
