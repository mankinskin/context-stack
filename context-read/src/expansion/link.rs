use context_trace::*;

#[derive(Debug, Clone)]
pub(crate) struct ExpansionLink {
    pub(crate) expansion_prefix: IndexStartPath,
    pub(crate) root_postfix: IndexEndPath,
    pub(crate) start_bound: usize,
}
