//! Compact formatting for cursor types

use crate::cursor::{
    CursorState,
    PathCursor,
};
use context_trace::logging::compact_format::{
    write_indent,
    CompactFormat,
};
use std::fmt;

impl<P: CompactFormat, S: CursorState> CompactFormat for PathCursor<P, S> {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "Cursor(")?;
        self.path.fmt_compact(f)?;
        write!(f, ", pos:{})", usize::from(self.atom_position))
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        writeln!(f, "PathCursor {{")?;
        write_indent(f, indent + 1)?;
        write!(f, "path: ")?;
        self.path.fmt_indented(f, indent + 1)?;
        writeln!(f, ",")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "position: {},", usize::from(self.atom_position))?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}
