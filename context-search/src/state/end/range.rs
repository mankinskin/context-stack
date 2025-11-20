use context_trace::{
    logging::compact_format::{
        write_indent,
        CompactFormat,
    },
    *,
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeEnd {
    pub(crate) path: IndexRangePath,
    pub(crate) target: DownKey,
    pub(crate) root_pos: AtomPosition,
    pub(crate) end_pos: AtomPosition,
}
impl LeafKey for RangeEnd {
    fn leaf_location(&self) -> ChildLocation {
        self.path.leaf_location()
    }
}

impl Traceable for &RangeEnd {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        RangeCommand::from(self).trace(ctx)
    }
}

impl From<&RangeEnd> for RangeCommand {
    fn from(value: &RangeEnd) -> Self {
        tracing::debug!(
            "Creating RangeCommand from RangeEnd: root_pos={}, end_pos={}",
            usize::from(value.root_pos),
            usize::from(value.end_pos)
        );
        RangeCommand {
            add_edges: true,
            path: value.path.clone(),
            root_pos: value.root_pos.into(),
            end_pos: value.end_pos,
        }
    }
}

impl CompactFormat for RangeEnd {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(
            f,
            "RangeEnd(root_pos:{}, end_pos:{})",
            usize::from(self.root_pos),
            usize::from(self.end_pos)
        )
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "RangeEnd {{")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "root_pos: {},", usize::from(self.root_pos))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "end_pos: {},", usize::from(self.end_pos))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "path: {:?},", &self.path)?;
        write_indent(f, indent + 1)?;
        writeln!(f, "target: {:?}", self.target)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}
