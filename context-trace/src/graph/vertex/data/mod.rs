//! VertexData module - Core vertex data structures and operations.
//!
//! Provides the central VertexData type used throughout the hypergraph,
//! along with operations for managing parent relationships and child patterns.

mod children;
mod core;
mod parents;

// Re-export main types
pub use core::{VertexData, VertexDataBuilder};

// Re-export helper functions
pub(crate) use children::{clone_child_patterns, localized_children_iter_for_index};

use crate::logging::compact_format::{write_indent, CompactFormat};

/// Display implementation for VertexData
impl std::fmt::Display for VertexData {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.fmt_indented(f, 0)
    }
}

/// CompactFormat implementation for VertexData
impl CompactFormat for VertexData {
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(
            f,
            "Vertex({}, w:{}, {}p, {}c)",
            self.index,
            self.width,
            self.parents.len(),
            self.children.len()
        )
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        indent: usize,
    ) -> std::fmt::Result {
        writeln!(f)?;
        write_indent(f, indent)?;
        writeln!(f, "Vertex {} {{", self.index)?;

        write_indent(f, indent + 1)?;
        writeln!(f, "width: {},", self.width)?;

        write_indent(f, indent + 1)?;
        writeln!(f, "parents: {} entries,", self.parents.len())?;

        write_indent(f, indent + 1)?;
        writeln!(f, "children: {} patterns", self.children.len())?;

        write_indent(f, indent)?;
        write!(f, "}}")
    }
}
