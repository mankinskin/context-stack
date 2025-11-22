use crate::{
    HashMap,
    graph::vertex::{
        VertexIndex,
        has_vertex_index::HasVertexIndex,
        token::Token,
    },
    logging::compact_format::{
        CompactFormat,
        write_indent,
    },
    trace::cache::{
        key::props::TargetKey,
        position::PositionCache,
        vertex::VertexCache,
    },
};
use derive_more::derive::IntoIterator;
use key::directed::DirectedKey;
use new::EditKind;
use std::fmt;

pub mod key;
pub mod new;
pub mod position;
pub mod vertex;

pub type StateDepth = usize;

#[derive(Clone, Debug, PartialEq, Eq, Default, IntoIterator)]
pub struct TraceCache {
    pub entries: HashMap<VertexIndex, VertexCache>,
}
impl TraceCache {
    pub fn new(start_index: Token) -> Self {
        let mut entries = HashMap::default();
        entries.insert(
            start_index.vertex_index(),
            VertexCache::start(start_index),
        );
        Self { entries }
    }
    pub fn add_state<E: Into<EditKind>>(
        &mut self,
        edit: E,
        add_edges: bool,
    ) -> (DirectedKey, bool) {
        let edit = edit.into();
        let key = edit.target_key();
        tracing::debug!(
            "add_state: index={}, pos={:?}",
            key.index,
            key.pos
        );
        if let Some(ve) = self.entries.get_mut(&key.index.vertex_index()) {
            if ve.get_mut(&key.pos).is_some() {
                (key, false)
            } else {
                let pe = PositionCache::build_edge(self, edit, add_edges);
                let ve =
                    self.entries.get_mut(&key.index.vertex_index()).unwrap();
                ve.insert(&key.pos, pe);
                (key, true)
            }
        } else {
            self.new_entry(key.clone(), edit, add_edges);
            (key, true)
        }
    }
    fn new_entry(
        &mut self,
        key: DirectedKey,
        edit: EditKind,
        add_edges: bool,
    ) {
        let mut ve = VertexCache::from(key.index);
        let pe = PositionCache::build_edge(self, edit, add_edges);
        ve.insert(&key.pos, pe);
        self.entries.insert(key.index.vertex_index(), ve);
    }
    pub(crate) fn force_mut(
        &mut self,
        key: &DirectedKey,
    ) -> &mut PositionCache {
        if !self.exists(key) {
            //let pe = PositionCache::start(key.index.clone());
            let pe = PositionCache::default();
            if let Some(ve) = self.get_vertex_mut(&key.index) {
                ve.insert(&key.pos, pe);
            } else {
                let mut ve = VertexCache::from(key.index);
                ve.insert(&key.pos, pe);
                self.entries.insert(key.index.vertex_index(), ve);
            }
        }
        self.expect_mut(key)
    }
    #[allow(dead_code)]
    pub(crate) fn get_vertex(
        &self,
        key: &Token,
    ) -> Option<&VertexCache> {
        self.entries.get(&key.index.vertex_index())
    }
    pub(crate) fn get_vertex_mut(
        &mut self,
        key: &Token,
    ) -> Option<&mut VertexCache> {
        self.entries.get_mut(&key.index.vertex_index())
    }
    #[allow(dead_code)]
    pub(crate) fn expect_vertex(
        &self,
        key: &Token,
    ) -> &VertexCache {
        self.get_vertex(key).unwrap()
    }
    #[allow(dead_code)]
    pub(crate) fn expect_vertex_mut(
        &mut self,
        key: &Token,
    ) -> &mut VertexCache {
        self.get_vertex_mut(key).unwrap()
    }
    #[allow(dead_code)]
    pub(crate) fn get(
        &self,
        key: &DirectedKey,
    ) -> Option<&PositionCache> {
        self.get_vertex(&key.index).and_then(|ve| ve.get(&key.pos))
    }
    pub(crate) fn get_mut(
        &mut self,
        key: &DirectedKey,
    ) -> Option<&mut PositionCache> {
        self.get_vertex_mut(&key.index)
            .and_then(|ve| ve.get_mut(&key.pos))
    }
    #[allow(dead_code)]
    pub(crate) fn expect(
        &self,
        key: &DirectedKey,
    ) -> &PositionCache {
        self.get(key).unwrap()
    }
    pub(crate) fn expect_mut(
        &mut self,
        key: &DirectedKey,
    ) -> &mut PositionCache {
        self.get_mut(key).unwrap()
    }
    #[allow(dead_code)]
    pub(crate) fn exists_vertex(
        &self,
        key: &Token,
    ) -> bool {
        self.entries.contains_key(&key.vertex_index())
    }
    pub(crate) fn exists(
        &self,
        key: &DirectedKey,
    ) -> bool {
        if let Some(ve) = self.entries.get(&key.index.vertex_index()) {
            ve.get(&key.pos).is_some()
        } else {
            false
        }
    }
}

impl Extend<(VertexIndex, VertexCache)> for TraceCache {
    fn extend<T: IntoIterator<Item = (VertexIndex, VertexCache)>>(
        &mut self,
        iter: T,
    ) {
        for (k, v) in iter {
            if let Some(c) = self.entries.get_mut(&k) {
                assert!(c.index == v.index);
                c.bottom_up.extend(v.bottom_up);
                c.top_down.extend(v.top_down);
            } else {
                self.entries.insert(k, v);
            }
        }
    }
}

impl CompactFormat for TraceCache {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "TraceCache({} entries)", self.entries.len())
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        writeln!(f)?;
        write_indent(f, indent)?;
        writeln!(f, "TraceCache {{")?;

        // Collect and sort entries by vertex index for consistent output
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_key(|(idx, _)| idx.0);

        for (vertex_idx, vertex_cache) in entries {
            write_indent(f, indent + 1)?;
            // VertexIndex Display will use test graph string representation
            writeln!(f, "{}: {{", vertex_idx)?;

            // Format the vertex cache contents
            write_indent(f, indent + 2)?;
            writeln!(f, "index: {},", vertex_idx)?;
            write_indent(f, indent + 2)?;
            writeln!(
                f,
                "bottom_up: {} entries,",
                vertex_cache.bottom_up.len()
            )?;
            write_indent(f, indent + 2)?;
            writeln!(f, "top_down: {} entries", vertex_cache.top_down.len())?;

            write_indent(f, indent + 1)?;
            writeln!(f, "}},")?;
        }

        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

impl fmt::Display for TraceCache {
    fn fmt(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        self.fmt_indented(f, 0)
    }
}
