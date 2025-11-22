use crate::{
    End,
    PathRole,
    graph::vertex::location::child::ChildLocation,
    path::structs::{
        role_path::RolePath,
        rooted::{
            RootedRangePath,
            index_range::IndexRangePath,
            pattern_range::PatternRangePath,
            role_path::RootedRolePath,
            root::PathRoot,
        },
        sub_path::{
            PositionAnnotated,
            SubPath,
        },
    },
};

/// move path leaf position one level deeper
/// The type parameter T represents what is being appended (typically ChildLocation or PositionAnnotated<ChildLocation>)
pub trait PathAppend<T = ChildLocation> {
    fn path_append(
        &mut self,
        entry: T,
    );
}

// Implementations for ChildLocation (original behavior)
impl<Role: PathRole, Root: PathRoot> PathAppend<ChildLocation>
    for RootedRolePath<Role, Root, ChildLocation>
{
    fn path_append(
        &mut self,
        parent_entry: ChildLocation,
    ) {
        self.role_path.sub_path.path_append(parent_entry);
    }
}

impl PathAppend<ChildLocation> for SubPath<ChildLocation> {
    fn path_append(
        &mut self,
        parent_entry: ChildLocation,
    ) {
        self.path.push(parent_entry)
    }
}

impl PathAppend<ChildLocation> for RolePath<End, ChildLocation> {
    fn path_append(
        &mut self,
        parent_entry: ChildLocation,
    ) {
        self.sub_path.path.push(parent_entry)
    }
}

impl PathAppend<ChildLocation> for IndexRangePath {
    fn path_append(
        &mut self,
        parent_entry: ChildLocation,
    ) {
        self.end.sub_path.path.push(parent_entry);
    }
}

impl PathAppend<ChildLocation> for PatternRangePath {
    fn path_append(
        &mut self,
        parent_entry: ChildLocation,
    ) {
        self.end.sub_path.path.push(parent_entry);
    }
}

// Implementations for PositionAnnotated<ChildLocation>
impl<Role: PathRole, Root: PathRoot>
    PathAppend<PositionAnnotated<ChildLocation>>
    for RootedRolePath<Role, Root, PositionAnnotated<ChildLocation>>
{
    fn path_append(
        &mut self,
        entry: PositionAnnotated<ChildLocation>,
    ) {
        self.role_path.sub_path.path_append(entry);
    }
}

impl PathAppend<PositionAnnotated<ChildLocation>>
    for SubPath<PositionAnnotated<ChildLocation>>
{
    fn path_append(
        &mut self,
        entry: PositionAnnotated<ChildLocation>,
    ) {
        self.path.push(entry)
    }
}

impl PathAppend<PositionAnnotated<ChildLocation>>
    for RolePath<End, PositionAnnotated<ChildLocation>>
{
    fn path_append(
        &mut self,
        entry: PositionAnnotated<ChildLocation>,
    ) {
        self.sub_path.path.push(entry)
    }
}

// Implementations for RootedRangePath with position-annotated end paths
impl<Root: PathRoot> PathAppend<PositionAnnotated<ChildLocation>>
    for RootedRangePath<Root, ChildLocation, PositionAnnotated<ChildLocation>>
{
    fn path_append(
        &mut self,
        entry: PositionAnnotated<ChildLocation>,
    ) {
        self.end.sub_path.path.push(entry);
    }
}

// Also implement PathAppend<ChildLocation> for compatibility with MovePath's default implementation
impl<Root: PathRoot> PathAppend<ChildLocation>
    for RootedRangePath<Root, ChildLocation, PositionAnnotated<ChildLocation>>
{
    fn path_append(
        &mut self,
        entry: ChildLocation,
    ) {
        // Wrap ChildLocation in PositionAnnotated with default position
        // Note: The position should be properly set by the caller (e.g., ChildState)
        self.end.sub_path.path.push(PositionAnnotated {
            node: entry,
            position: Default::default(),
        });
    }
}
