use context_trace::{
    logging::compact_format::{
        write_indent,
        CompactFormat,
    },
    trace::cache::key::directed::down::DownPosition,
    *,
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrefixEnd {
    pub(crate) path: IndexEndPath,
    pub(crate) target: DownKey,
    pub(crate) exit_pos: DownPosition,
    pub(crate) end_pos: AtomPosition,
}
impl From<&PrefixEnd> for PrefixCommand {
    fn from(value: &PrefixEnd) -> Self {
        PrefixCommand::new(value.path.clone(), value.exit_pos)
    }
}
impl Traceable for &PrefixEnd {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        PrefixCommand::from(self).trace(ctx)
    }
}

impl CompactFormat for PrefixEnd {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(
            f,
            "PrefixEnd(exit_pos:{}, end_pos:{})",
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
        writeln!(f, "PrefixEnd {{")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "root_pos: {},", usize::from(self.exit_pos.0))?;
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
