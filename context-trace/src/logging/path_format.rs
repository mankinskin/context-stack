//! Compact formatting implementations for path types

use crate::{
    BaseState,
    ChildLocation,
    ChildState,
    IndexRangePath,
    ParentState,
    PathRole,
    graph::vertex::{
        location::pattern::PatternLocation,
        pattern::Pattern,
    },
    impl_display_via_compact,
    logging::compact_format::{
        CompactFormat,
        write_indent,
    },
    path::structs::{
        role_path::RolePath,
        rooted::{
            PathNode,
            RootedRangePath,
            role_path::{
                PatternEndPath,
                RootedRolePath,
            },
            root::{
                IndexRoot,
                PathRoot,
                RootedPath,
            },
        },
    },
};
use std::fmt;

// CompactFormat for RootedRangePath<Pattern> (which is Vec<Token>)
impl CompactFormat for RootedRangePath<Pattern> {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let start_entry = self.start.sub_path.root_entry;
        let end_entry = self.end.sub_path.root_entry;

        write!(f, "Pattern")?;
        // Use Display trait for Pattern which uses Token's Display for each token
        write!(f, "{}", &self.root)?;
        write!(f, "[{}..{}]", start_entry, end_entry)?;
        Ok(())
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        writeln!(f, "PatternRangePath {{")?;

        // Root pattern
        write_indent(f, indent + 1)?;
        write!(f, "pattern: ")?;
        // Use Display trait for Pattern which uses Token's Display for each token
        write!(f, "{}", &self.root)?;
        writeln!(f, ",")?;

        // Start position
        write_indent(f, indent + 1)?;
        writeln!(f, "start: {},", self.start)?;

        // End position
        write_indent(f, indent + 1)?;
        write!(f, "end: {}", self.end)?;
        writeln!(f)?;

        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

// CompactFormat for generic RootedRangePath<IndexRoot, ChildLocation, EndNode>
impl<EndNode> CompactFormat
    for RootedRangePath<IndexRoot, ChildLocation, EndNode>
where
    EndNode: PathNode,
{
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let start_entry = self.start.sub_path.root_entry;
        write!(f, "Index({})[{}..]", self.root, start_entry)?;
        Ok(())
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        writeln!(f, "IndexRangePath {{")?;

        // Root
        write_indent(f, indent + 1)?;
        writeln!(f, "root: {},", self.root)?;

        // Start position
        write_indent(f, indent + 1)?;
        writeln!(f, "start: {},", self.start)?;

        // End position
        write_indent(f, indent + 1)?;
        write!(f, "end: {:?}", self.end)?;
        writeln!(f)?;

        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

// CompactFormat for PatternLocation
impl CompactFormat for PatternLocation {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "{}", self)
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        write!(
            f,
            "PatternLocation({}, {})",
            self.parent,
            &self.pattern_id.to_string()[..8]
        )
    }
}

// CompactFormat for IndexRoot
impl CompactFormat for IndexRoot {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "{}", self)
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        write!(f, "{}", self)
    }
}

// CompactFormat for RootedRolePath (PatternEndPath = RootedRolePath<End, Pattern>)
impl CompactFormat for PatternEndPath {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "PatternEnd(")?;
        // Use Display trait for Pattern which uses Token's Display for each token
        write!(f, "{}", &self.root)?;
        write!(f, ", {})", self.role_path)
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        writeln!(f, "PatternEndPath {{")?;
        write_indent(f, indent + 1)?;
        write!(f, "pattern: ")?;
        // Use Display trait for Pattern which uses Token's Display for each token
        write!(f, "{}", &self.root)?;
        writeln!(f, ",")?;
        write_indent(f, indent + 1)?;
        write!(f, "{}", self.role_path)?;
        writeln!(f)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

// CompactFormat for ChildState
impl<EndNode> CompactFormat for ChildState<EndNode>
where
    EndNode: PathNode,
    IndexRangePath<ChildLocation, EndNode>: CompactFormat,
{
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let entry_pos: usize = self.entry_pos.0.into();
        let exit_pos: usize = self.exit_pos.0.into();
        write!(
            f,
            "ChildState(entry_pos:{}, exit_pos:{}, path:...)",
            entry_pos, exit_pos,
        )
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        writeln!(f, "ChildState {{")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "entry_pos: {},", usize::from(self.entry_pos.0))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "exit_pos: {},", usize::from(self.exit_pos.0))?;
        write_indent(f, indent + 1)?;
        write!(f, "path: ")?;
        self.path.fmt_indented(f, indent + 1)?;
        writeln!(f)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

impl<P: RootedPath> CompactFormat for crate::trace::state::BaseState<P>
where
    P: CompactFormat,
{
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let prev_pos: usize = self.prev_pos.into();
        let root_pos: usize = self.root_pos.into();
        write!(f, "BaseState(prev:{}, root:{})", prev_pos, root_pos)
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "BaseState {{")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "prev_pos: {},", usize::from(self.prev_pos))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "root_pos: {},", usize::from(self.root_pos))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "path:")?;
        self.path.fmt_indented(f, indent + 2)?;
        writeln!(f)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

// CompactFormat for ParentState
impl CompactFormat for ParentState {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let prev_pos: usize = self.prev_pos.0.into();
        let root_pos: usize = self.root_pos.0.into();
        write!(f, "ParentState(prev:{}, root:{})", prev_pos, root_pos)
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "ParentState {{")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "prev_pos: {},", usize::from(self.prev_pos.0))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "root_pos: {},", usize::from(self.root_pos.0))?;
        write_indent(f, indent + 1)?;
        writeln!(f, "path:")?;
        self.path.fmt_indented(f, indent + 2)?;
        writeln!(f)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

impl<R: PathRole> CompactFormat for RolePath<R> {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(
            f,
            "RolePath(entry:{}, len:{})",
            self.sub_path.root_entry,
            self.sub_path.path.len()
        )
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "RolePath {{")?;
        write_indent(f, indent + 1)?;
        write!(f, "{}", self)?;
        writeln!(f)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

impl<R: PathRole, Root: PathRoot> CompactFormat for RootedRolePath<R, Root>
where
    Root: CompactFormat,
{
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "RootedRolePath(")?;
        self.root.fmt_compact(f)?;
        if !self.role_path.is_empty() {
            write!(f, ", path_len:{})", self.role_path.len())?;
        } else {
            write!(f, ")")?;
        }
        Ok(())
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "RootedRolePath {{")?;
        write_indent(f, indent + 1)?;
        write!(f, "root: ")?;
        self.root.fmt_compact(f)?;
        writeln!(f, ",")?;
        write_indent(f, indent + 1)?;
        write!(f, "role_path: ")?;
        self.role_path.fmt_compact(f)?;
        writeln!(f)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

// Implement Display for types to enable % formatting in tracing without Compact wrapper
// Note: Only implement for types that don't already have Display
// (RootedRangePath and RolePath already have Display implemented directly)
impl_display_via_compact!(PatternEndPath);
impl_display_via_compact!(RootedRolePath<R, Root> where R: PathRole, Root: PathRoot + CompactFormat);
impl_display_via_compact!(BaseState<P> where P: RootedPath + CompactFormat);
impl_display_via_compact!(ChildState<EndNode> where EndNode: PathNode);
impl_display_via_compact!(ParentState);
