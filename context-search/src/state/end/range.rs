use context_trace::{
    logging::compact_format::{
        write_indent,
        CompactFormat,
    },
    trace::cache::key::directed::{
        down::DownPosition,
        up::UpPosition,
    },
    *,
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeEnd {
    pub(crate) path: IndexRangePath,
    pub(crate) target: DownKey,
    pub(crate) entry_pos: UpPosition,
    pub(crate) exit_pos: DownPosition,
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
        tracing::trace!(
            "Creating RangeCommand from RangeEnd: entry_pos={}, exit_pos={}, end_pos={}",
            usize::from(value.entry_pos.0),
            usize::from(value.exit_pos.0),
            usize::from(value.end_pos)
        );
        RangeCommand::new(value.path.clone(), value.entry_pos, value.exit_pos)
    }
}

impl CompactFormat for RangeEnd {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(
            f,
            "RangeEnd(entry_pos:{}, exit_pos:{}, end_pos:{})",
            usize::from(self.entry_pos.0),
            usize::from(self.exit_pos.0),
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
        writeln!(f, "entry_pos: {},", usize::from(self.entry_pos.0))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "exit_pos: {},", usize::from(self.exit_pos.0))?;
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
