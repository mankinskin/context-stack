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

    /// Mutable access to prev position
    fn prev_pos_mut(&mut self) -> &mut AtomPosition;

    /// Mutable access to root position
    fn root_pos_mut(&mut self) -> &mut AtomPosition;
}

/// Trait for types that have a target offset.
///
/// The target offset represents the **offset position before the target token**,
/// not including the token's width. It indicates where in the atom sequence
/// the target token begins.
///
/// For example, if matching pattern [a, b, c] and the target is token 'b':
/// - If 'a' has width 1, then target_offset = 1 (position before 'b')
/// - The position after 'b' would be target_offset + width(b)
pub trait HasTargetOffset: StatePosition {
    /// Get the offset position before the target token
    fn target_offset(&self) -> &AtomPosition;

    /// Get mutable access to the offset position before the target token
    fn target_offset_mut(&mut self) -> &mut AtomPosition;
}

/// Macro to implement StatePosition for types with prev_pos and root_pos fields
///
/// This macro reduces boilerplate for types that store position state.
///
/// # Basic usage
/// ```ignore
/// impl_state_position! {
///     for MyState => {
///         prev_pos: prev_pos,
///         root_pos: root_pos,
///     }
/// }
/// ```
///
/// # With generics
/// ```ignore
/// impl_state_position! {
///     for MyState<T> where [T: Clone] => {
///         prev_pos: prev_pos_field,
///         root_pos: root_pos_field,
///     }
/// }
/// ```
///
/// # With generic parameters
/// ```ignore
/// impl_state_position! {
///     for BaseState<P> where [P: RootedPath] => {
///         prev_pos: prev_pos,
///         root_pos: root_pos,
///     }
/// }
/// ```
#[macro_export]
macro_rules! impl_state_position {
    // Pattern without generics: for Type => { ... }
    (
        for $ty:ty => {
            prev_pos: $prev_field:ident,
            root_pos: $root_field:ident,
        }
    ) => {
        impl $crate::path::accessors::path_accessor::StatePosition for $ty {
            fn prev_pos(&self) -> &$crate::path::mutators::move_path::key::AtomPosition {
                &self.$prev_field.0
            }

            fn root_pos(&self) -> &$crate::path::mutators::move_path::key::AtomPosition {
                &self.$root_field.0
            }

            fn prev_pos_mut(&mut self) -> &mut $crate::path::mutators::move_path::key::AtomPosition {
                &mut self.$prev_field.0
            }

            fn root_pos_mut(&mut self) -> &mut $crate::path::mutators::move_path::key::AtomPosition {
                &mut self.$root_field.0
            }
        }
    };

    // Pattern with generics: for Type<G> where [bounds] => { ... }
    (
        for $ty:ty where [$($bounds:tt)*] => {
            prev_pos: $prev_field:ident,
            root_pos: $root_field:ident,
        }
    ) => {
        impl<$($bounds)*> $crate::path::accessors::path_accessor::StatePosition for $ty {
            fn prev_pos(&self) -> &$crate::path::mutators::move_path::key::AtomPosition {
                &self.$prev_field
            }

            fn root_pos(&self) -> &$crate::path::mutators::move_path::key::AtomPosition {
                &self.$root_field
            }

            fn prev_pos_mut(&mut self) -> &mut $crate::path::mutators::move_path::key::AtomPosition {
                &mut self.$prev_field
            }

            fn root_pos_mut(&mut self) -> &mut $crate::path::mutators::move_path::key::AtomPosition {
                &mut self.$root_field
            }
        }
    };

    // Pattern without generics with target_pos: for Type => { ...with target... }
    (
        for $ty:ty => {
            prev_pos: $prev_field:ident,
            root_pos: $root_field:ident,
            target_pos: Some($target_field:ident),
        }
    ) => {
        impl $crate::path::accessors::path_accessor::StatePosition for $ty {
            fn prev_pos(&self) -> &$crate::path::mutators::move_path::key::AtomPosition {
                &self.$prev_field
            }

            fn root_pos(&self) -> &$crate::path::mutators::move_path::key::AtomPosition {
                &self.$root_field
            }

            fn target_pos(&self) -> Option<&$crate::path::mutators::move_path::key::AtomPosition> {
                Some(&self.$target_field)
            }

            fn prev_pos_mut(&mut self) -> &mut $crate::path::mutators::move_path::key::AtomPosition {
                &mut self.$prev_field
            }

            fn root_pos_mut(&mut self) -> &mut $crate::path::mutators::move_path::key::AtomPosition {
                &mut self.$root_field
            }

            fn target_pos_mut(&mut self) -> Option<&mut $crate::path::mutators::move_path::key::AtomPosition> {
                Some(&mut self.$target_field)
            }
        }
    };

    // Pattern with generics and target_pos: for Type<G> where [bounds] => { ...with target... }
    (
        for $ty:ty where [$($bounds:tt)*] => {
            prev_pos: $prev_field:ident,
            root_pos: $root_field:ident,
            target_pos: Some($target_field:ident),
        }
    ) => {
        impl<$($bounds)*> $crate::path::accessors::path_accessor::StatePosition for $ty {
            fn prev_pos(&self) -> &$crate::path::mutators::move_path::key::AtomPosition {
                &self.$prev_field
            }

            fn root_pos(&self) -> &$crate::path::mutators::move_path::key::AtomPosition {
                &self.$root_field
            }

            fn target_pos(&self) -> Option<&$crate::path::mutators::move_path::key::AtomPosition> {
                Some(&self.$target_field)
            }

            fn prev_pos_mut(&mut self) -> &mut $crate::path::mutators::move_path::key::AtomPosition {
                &mut self.$prev_field
            }

            fn root_pos_mut(&mut self) -> &mut $crate::path::mutators::move_path::key::AtomPosition {
                &mut self.$root_field
            }

            fn target_pos_mut(&mut self) -> Option<&mut $crate::path::mutators::move_path::key::AtomPosition> {
                Some(&mut self.$target_field)
            }
        }
    };
}

pub use impl_state_position;
