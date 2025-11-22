/// Tier 2 path accessor traits for RolePath struct access
///
/// These traits provide access to the full RolePath structure (including root_entry field),
/// complementing the Tier 1 PathAccessor trait which only provides path vector access.
///
/// Use these when you need:
/// - Access to the root_entry field
/// - The complete RolePath structure
/// - Concrete role-specific access (Start vs End)
///
/// For simple path vector access, use PathAccessor instead.

use crate::{
    graph::vertex::location::child::ChildLocation,
    path::{
        accessors::{
            has_path::HasRolePath,
            role::{End, Start, PathRole},
        },
        structs::{
            role_path::RolePath,
            rooted::{
                role_path::RootedRolePath,
                root::{PathRoot, RootedPath},
            },
        },
    },
};

/// Access to Start role path structure
///
/// Provides the complete RolePath<Start> structure, including root_entry.
/// Implemented by types that contain or are Start role paths.
///
/// # Example
/// ```ignore
/// use context_trace::{StartPathAccessor, RolePath, Start};
/// 
/// fn get_start_entry<T: StartPathAccessor>(path: &T) -> usize {
///     path.start_path().root_entry
/// }
/// ```
pub trait StartPathAccessor {
    type Node;
    
    /// Get immutable reference to the Start role path
    fn start_path(&self) -> &RolePath<Start, Self::Node>;
    
    /// Get mutable reference to the Start role path
    fn start_path_mut(&mut self) -> &mut RolePath<Start, Self::Node>;
}

/// Access to End role path structure
///
/// Provides the complete RolePath<End> structure, including root_entry.
/// Implemented by types that contain or are End role paths.
///
/// # Example
/// ```ignore
/// use context_trace::{EndPathAccessor, RolePath, End};
/// 
/// fn get_end_entry<T: EndPathAccessor>(path: &T) -> usize {
///     path.end_path().root_entry
/// }
/// ```
pub trait EndPathAccessor {
    type Node;
    
    /// Get immutable reference to the End role path
    fn end_path(&self) -> &RolePath<End, Self::Node>;
    
    /// Get mutable reference to the End role path
    fn end_path_mut(&mut self) -> &mut RolePath<End, Self::Node>;
}

/// Combined access to both Start and End role paths
///
/// Marker trait for types that contain both Start and End paths (like RootedRangePath).
/// Automatically implemented for types implementing both StartPathAccessor and EndPathAccessor.
///
/// # Example
/// ```ignore
/// use context_trace::{RangePathAccessor, StartPathAccessor, EndPathAccessor};
/// 
/// fn compare_entries<T: RangePathAccessor>(range: &T) -> (usize, usize) {
///     (
///         range.start_path().root_entry,
///         range.end_path().root_entry
///     )
/// }
/// ```
pub trait RangePathAccessor: StartPathAccessor + EndPathAccessor {}

// Blanket implementation for any type implementing both role accessors
impl<T> RangePathAccessor for T where T: StartPathAccessor + EndPathAccessor {}

/// Consolidated access to a rooted Start role path
///
/// Combines RootedPath + HasRolePath<Start> to provide complete access to a rooted
/// Start path including the root and the role path structure.
///
/// This trait is automatically implemented for any type that implements both
/// `RootedPath` and `HasRolePath<Start, Node = ChildLocation>`.
///
/// **When to use:**
/// - Need both root and Start role path access
/// - Working with types like RootedStartPath or PostfixEnd
/// - Want a single trait bound for rooted Start path capabilities
///
/// **Replaces:** `HasRootedRolePath<Root, Start>` for better clarity
///
/// # Example
/// ```ignore
/// use context_trace::{RootedStartPathAccessor, RolePath, Start};
/// 
/// fn process_rooted_start<T: RootedStartPathAccessor>(path: &T) {
///     let root = path.path_root();
///     let role_path = path.start_role_path();
///     let complete = path.rooted_start_path();
/// }
/// ```
pub trait RootedStartPathAccessor: RootedPath + HasRolePath<Start, Node = ChildLocation> {
    /// Get the complete rooted Start path
    fn rooted_start_path(&self) -> RootedRolePath<Start, Self::Root, ChildLocation> {
        let root = self.path_root();
        self.role_path().clone().into_rooted(root)
    }
    
    /// Get reference to the Start role path (convenience method)
    fn start_role_path(&self) -> &RolePath<Start, ChildLocation> {
        self.role_path()
    }
    
    /// Get mutable reference to the Start role path (convenience method)
    fn start_role_path_mut(&mut self) -> &mut RolePath<Start, ChildLocation> {
        self.role_path_mut()
    }
}

/// Consolidated access to a rooted End role path
///
/// Combines RootedPath + HasRolePath<End> to provide complete access to a rooted
/// End path including the root and the role path structure.
///
/// This trait is automatically implemented for any type that implements both
/// `RootedPath` and `HasRolePath<End, Node = ChildLocation>`.
///
/// **When to use:**
/// - Need both root and End role path access
/// - Working with types like RootedEndPath
/// - Want a single trait bound for rooted End path capabilities
///
/// **Replaces:** `HasRootedRolePath<Root, End>` for better clarity
///
/// # Example
/// ```ignore
/// use context_trace::{RootedEndPathAccessor, RolePath, End};
/// 
/// fn process_rooted_end<T: RootedEndPathAccessor>(path: &T) {
///     let root = path.path_root();
///     let role_path = path.end_role_path();
///     let complete = path.rooted_end_path();
/// }
/// ```
pub trait RootedEndPathAccessor: RootedPath + HasRolePath<End, Node = ChildLocation> {
    /// Get the complete rooted End path
    fn rooted_end_path(&self) -> RootedRolePath<End, Self::Root, ChildLocation> {
        let root = self.path_root();
        self.role_path().clone().into_rooted(root)
    }
    
    /// Get reference to the End role path (convenience method)
    fn end_role_path(&self) -> &RolePath<End, ChildLocation> {
        self.role_path()
    }
    
    /// Get mutable reference to the End role path (convenience method)
    fn end_role_path_mut(&mut self) -> &mut RolePath<End, ChildLocation> {
        self.role_path_mut()
    }
}

/// Generic rooted role path accessor for any role
///
/// Provides role-generic access to rooted role paths. Use the concrete
/// RootedStartPathAccessor or RootedEndPathAccessor when the role is known.
///
/// This trait is automatically implemented for any type that implements both
/// `RootedPath` and `HasRolePath<R, Node = ChildLocation>`.
pub trait RootedRolePathAccessor<R: PathRole>: RootedPath + HasRolePath<R, Node = ChildLocation> {
    /// Get the complete rooted role path
    fn rooted_role_path(&self) -> RootedRolePath<R, Self::Root, ChildLocation> {
        let root = self.path_root();
        self.role_path().clone().into_rooted(root)
    }
}

// Blanket implementations for automatic trait satisfaction
impl<T> RootedStartPathAccessor for T 
where 
    T: RootedPath + HasRolePath<Start, Node = ChildLocation> 
{}

impl<T> RootedEndPathAccessor for T 
where 
    T: RootedPath + HasRolePath<End, Node = ChildLocation> 
{}

impl<R: PathRole, T> RootedRolePathAccessor<R> for T 
where 
    T: RootedPath + HasRolePath<R, Node = ChildLocation> 
{}
