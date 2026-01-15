use context_insert::*;
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use derive_new::new;

#[derive(Debug, Clone, Deref, DerefMut, new)]
pub struct RootManager {
    #[deref]
    #[deref_mut]
    pub graph: HypergraphRef,
    #[new(default)]
    pub root: Option<Token>,
}

impl RootManager {
    /// append a pattern of new atom indices
    /// returns index of possible new index
    pub fn append_pattern(
        &mut self,
        new: Pattern,
    ) {
        match new.len() {
            0 => {},
            1 => {
                let new = new.first().unwrap();
                self.append_index(new)
            },
            _ => {
                if let Some(root) = &mut self.root {
                    let vertex = (*root).vertex(&self.graph);
                    *root = if vertex.child_patterns().len() == 1
                        && vertex.parents().is_empty()
                    {
                        let (&pid, _) = vertex.expect_any_child_pattern();
                        self.graph.append_to_pattern(*root, pid, new)
                    } else {
                        // some old overlaps though
                        let new = new.into_pattern();
                        self.graph
                            .insert_pattern([&[*root], new.as_slice()].concat())
                    };
                } else {
                    let c = self.graph.insert_pattern(new);
                    self.root = Some(c);
                }
            },
        }
    }
    #[context_trace::instrument_sig(skip(self, index))]
    pub fn append_index(
        &mut self,
        index: impl ToToken,
    ) {
        let index = index.to_child();
        if let Some(root) = &mut self.root {
            let vertex = (*root).vertex(&self.graph);
            *root = if index.vertex_index() != root.vertex_index()
                && vertex.child_patterns().len() == 1
                && vertex.parents().is_empty()
            {
                let (&pid, _) = vertex.expect_any_child_pattern();
                self.graph.append_to_pattern(*root, pid, index)
            } else {
                self.graph.insert_pattern(vec![*root, index])
            };
        } else {
            self.root = Some(index);
        }
    }
}

// RootManager derefs to HypergraphRef, which implements HasGraph
impl_has_graph! {
    impl for RootManager,
    self => &**self;
    <'a> &'a Hypergraph
}
impl<R: InsertResult> ToInsertCtx<R> for RootManager {
    fn insert_context(&self) -> InsertCtx<R> {
        InsertCtx::from(self.graph.clone())
    }
}
