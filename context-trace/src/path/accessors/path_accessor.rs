/// New consolidated path accessor traits (Phase 1 refactoring)
///
/// These traits replace the fragmented Has* trait hierarchy with a cleaner,
/// more discoverable API. The old traits are still available but deprecated.
use crate::path::mutators::move_path::key::AtomPosition;

/// Core path accessor trait - provides access to the path vector
///
/// Replaces: `HasPath<R>`, `HasRolePath<R>` for most use cases
///
/// This trait provides mutable and immutable access to a path's node vector.
/// It's parameterized by the path role (Start/End) through an associated type.
pub trait PathAccessor {
    /// The role of this path (Start or End)
    type Role: crate::path::accessors::role::PathRole;

    /// The node type in the path (usually ChildLocation)
    type Node;

    /// Get immutable reference to the path vector
    fn path(&self) -> &Vec<Self::Node>;

    /// Get mutable reference to the path vector
    fn path_mut(&mut self) -> &mut Vec<Self::Node>;
}

/// Extension for rooted paths - provides access to the root
///
/// Replaces: `RootedPath` + `HasPath` combinations
///
/// This trait extends PathAccessor for paths that have a defined root token.
/// It provides access to both the path and its root.
pub trait RootedPathAccessor: PathAccessor {
    /// The root type (e.g., IndexRoot, Pattern)
    type Root: crate::path::structs::rooted::root::PathRoot;

    /// Get the root of this path
    fn get_root(&self) -> Self::Root;

    /// Get mutable reference to root (if supported)
    fn get_root_mut(&mut self) -> &mut Self::Root
    where
        Self::Root: Clone,
    {
        // Default implementation for types that don't support root mutation
        // Can be overridden for types that do
        unimplemented!("Root mutation not supported for this type")
    }
}

/// Unified position accessor for trace states
///
/// Replaces: `HasPrevPos`, `HasRootPos`, `HasTargetPos`
///
/// This trait consolidates the three separate position accessor traits into
/// a single trait, making it easier to work with state positions.
pub trait StatePosition {
    /// Position before entering the current token
    fn prev_pos(&self) -> &AtomPosition;

    /// Position of the root token
    fn root_pos(&self) -> &AtomPosition;

    /// Target position (if applicable - None for states without targets)
    fn target_pos(&self) -> Option<&AtomPosition> {
        None
    }

    /// Mutable access to prev position
    fn prev_pos_mut(&mut self) -> &mut AtomPosition;

    /// Mutable access to root position
    fn root_pos_mut(&mut self) -> &mut AtomPosition;

    /// Mutable access to target position (if applicable)
    fn target_pos_mut(&mut self) -> Option<&mut AtomPosition> {
        None
    }
}
