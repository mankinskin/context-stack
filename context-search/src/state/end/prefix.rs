use context_trace::{
    logging::compact_format::{
        write_indent,
        CompactFormat,
    },
    *,
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefixEnd {
    pub(crate) path: IndexEndPath,
    pub(crate) target: DownKey,
    pub(crate) root_pos: AtomPosition,
    pub(crate) end_pos: AtomPosition,
}
impl From<&PrefixEnd> for PrefixCommand {
    fn from(value: &PrefixEnd) -> Self {
        PrefixCommand {
            path: value.path.clone(),
            root_pos: value.root_pos,
            end_pos: value.end_pos,
        }
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
            "PrefixEnd(root_pos:{}, end_pos:{})",
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
        writeln!(f, "PrefixEnd {{")?;
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
