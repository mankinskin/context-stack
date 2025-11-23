//! Position annotation methods for IndexRangePath

use crate::{
    TokenWidth,
    graph::vertex::{
        location::child::ChildLocation,
        pattern::{
            pattern_pre_ctx,
            pattern_width,
        },
        token::Token,
        wide::Wide,
    },
    path::{
        accessors::{
            has_path::{
                HasPath,
                HasRolePath,
            },
            role::{
                End,
                PathRole,
            },
        },
        mutators::move_path::key::{
            AdvanceKey,
            AtomPosition,
        },
        structs::{
            role_path::RolePath,
            rooted::role_path::{
                HasRootChildIndex,
                HasRootChildToken,
            },
            sub_path::PositionAnnotated,
        },
    },
    trace::has_graph::HasGraph,
};

use super::IndexRangePath;

impl IndexRangePath<ChildLocation, ChildLocation> {
    /// Convert to position-annotated path, accumulating widths as we traverse
    ///
    /// Starting from `entry_position`, walks through each child location in the end path,
    /// calculating the position at which we entered that child by adding the widths of
    /// all preceding tokens in the parent pattern.
    pub fn with_positions<G: HasGraph>(
        self,
        entry_position: AtomPosition,
        trav: &G,
    ) -> IndexRangePath<ChildLocation, PositionAnnotated<ChildLocation>> {
        let graph = trav.graph();
        let mut current_position = entry_position;

        let annotated_path: Vec<PositionAnnotated<ChildLocation>> = self
            .end
            .path()
            .iter()
            .map(|&loc| {
                // Get the pattern at this location
                let pattern = graph.expect_pattern_at(loc);

                // The position at which we enter this child
                let entry_pos = current_position;

                // Calculate width of tokens before the child we're entering
                let width_before =
                    pattern_width(pattern_pre_ctx(pattern, loc.sub_index));

                // Update position for next iteration: add width before child + width of child token
                current_position.advance_key(
                    (width_before + pattern[loc.sub_index].width()).0,
                );

                PositionAnnotated::new(loc, entry_pos)
            })
            .collect();

        let annotated_end =
            RolePath::new(self.end.root_child_index(), annotated_path);
        IndexRangePath::new(self.root, self.start, annotated_end)
    }
}

impl IndexRangePath<ChildLocation, PositionAnnotated<ChildLocation>> {
    /// Get the leaf ChildLocation from a position-annotated path
    /// Unwraps the position annotation to access the underlying location
    pub fn leaf_location(&self) -> Option<ChildLocation> {
        self.end.path().last().map(|annotated| annotated.node)
    }

    /// Convert position-annotated path to plain ChildLocation path
    /// This strips the position information, keeping only the locations
    pub fn into_plain(self) -> IndexRangePath<ChildLocation, ChildLocation> {
        let plain_end_path: Vec<ChildLocation> = self
            .end
            .path()
            .iter()
            .map(|annotated| annotated.node)
            .collect();
        IndexRangePath {
            root: self.root,
            start: self.start,
            end: RolePath::new(self.end.root_child_index(), plain_end_path),
        }
    }

    /// Get the rooted leaf token from position-annotated end path
    /// Extracts the ChildLocation and retrieves the token
    pub fn role_rooted_leaf_token<R: PathRole, G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token
    where
        Self: HasRolePath<R, Node = PositionAnnotated<ChildLocation>>
            + HasRootChildIndex<R>
            + HasRootChildToken<R>,
    {
        // Extract leaf location from position-annotated node
        let role_path = self.role_path();
        let leaf_loc = role_path
            .path()
            .last()
            .map(|annotated| annotated.node)
            .unwrap_or_else(|| {
                self.root.location.to_child_location(role_path.root_entry)
            });

        // Get token at that location, or fall back to root child token
        trav.graph()
            .get_child_at(leaf_loc)
            .copied()
            .unwrap_or(self.root_child_token(trav))
    }
}
