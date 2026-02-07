use context_trace::*;
use derive_more::derive::{
    Deref,
    DerefMut,
};
use std::fmt;
#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub(crate) struct PostfixEnd {
    #[deref]
    #[deref_mut]
    pub(crate) path: IndexStartPath,
    pub(crate) entry_pos: UpPosition,
}
// HasRootPos implementation removed - use StatePosition instead if needed
impl RootedPath for PostfixEnd {
    type Root = IndexRoot;
    fn path_root(&self) -> IndexRoot {
        self.path.path_root()
    }
}
impl IntoRolePath<Start> for PostfixEnd {
    fn into_role_path(self) -> RolePath<Start> {
        self.path.into_role_path()
    }
}
impl IntoRootedRolePath<Start> for PostfixEnd {
    fn into_rooted_role_path(self) -> IndexStartPath {
        self.path
    }
}
// PostfixEnd automatically implements RootedStartPathAccessor via blanket impl
// (it implements RootedPath + HasRolePath<Start, Node = ChildLocation>)

impl Traceable for &'_ PostfixEnd {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        PostfixCommand::from(self).trace(ctx)
    }
}

impl From<&'_ PostfixEnd> for PostfixCommand {
    fn from(value: &'_ PostfixEnd) -> Self {
        tracing::trace!(
            "Creating PostfixCommand from PostfixEnd: entry_pos={}",
            usize::from(value.entry_pos.0)
        );
        PostfixCommand::new(
            value.path.clone(),
            value.path.role_root_child_location::<Start>(),
            value.entry_pos,
        )
    }
}

impl CompactFormat for PostfixEnd {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "PostfixEnd(root_pos:{})", usize::from(self.entry_pos.0))
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "PostfixEnd {{")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "entry_pos: {},", usize::from(self.entry_pos.0))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "path: {:?}", &self.path)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}
