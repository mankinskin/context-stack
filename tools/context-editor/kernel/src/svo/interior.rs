use super::*;

impl VoxelWorld {
    /// Check whether the voxel at `pos` is occupied (non-zero color_data).
    /// Out-of-bounds positions are treated as empty.
    pub fn is_occupied_at(
        &self,
        pos: IVec3,
    ) -> bool {
        let max_c = (1i32 << self.max_depth) - 1;
        if pos.x < 0
            || pos.y < 0
            || pos.z < 0
            || pos.x > max_c
            || pos.y > max_c
            || pos.z > max_c
        {
            return false;
        }
        self.descend_to(pos)
            .map(|idx| self.nodes[idx].color_data != 0)
            .unwrap_or(false)
    }

    fn all_face_neighbors_occupied(
        &self,
        pos: IVec3,
    ) -> bool {
        for &dir in &[
            IVec3::X,
            -IVec3::X,
            IVec3::Y,
            -IVec3::Y,
            IVec3::Z,
            -IVec3::Z,
        ] {
            if !self.is_occupied_at(pos + dir) {
                return false;
            }
        }
        true
    }

    pub(super) fn reclassify_interior_around(
        &mut self,
        pos: IVec3,
    ) {
        let dirs = [
            IVec3::ZERO,
            IVec3::X,
            -IVec3::X,
            IVec3::Y,
            -IVec3::Y,
            IVec3::Z,
            -IVec3::Z,
        ];
        let max_c = (1i32 << self.max_depth) - 1;

        let updates: Vec<(usize, bool)> = dirs
            .iter()
            .filter_map(|&dir| {
                let candidate = pos + dir;
                if candidate.x < 0
                    || candidate.y < 0
                    || candidate.z < 0
                    || candidate.x > max_c
                    || candidate.y > max_c
                    || candidate.z > max_c
                {
                    return None;
                }
                let idx = self.descend_to(candidate)?;
                if self.nodes[idx].color_data == 0 {
                    return None;
                }
                let interior = self.all_face_neighbors_occupied(candidate);
                Some((idx, interior))
            })
            .collect();

        for (idx, interior) in updates {
            if interior {
                self.nodes[idx].child_pointer |= OctreeNode::INTERIOR_FLAG;
            } else {
                self.nodes[idx].child_pointer &= !OctreeNode::INTERIOR_FLAG;
            }
            self.mark_dirty(idx);
        }
    }

    /// One-shot pass to classify every leaf node as interior or surface.
    pub fn recompute_all_interior_flags(&mut self) {
        let node_count = self.nodes.len();
        let max_c = (1i32 << self.max_depth) - 1;

        let mut leaves: Vec<(usize, IVec3)> = Vec::new();
        self.collect_occupied_leaves(
            self.root_index as usize,
            IVec3::ZERO,
            1i32 << self.max_depth,
            &mut leaves,
        );
        let _ = (node_count, max_c);

        let updates: Vec<(usize, bool)> = leaves
            .iter()
            .map(|&(idx, pos)| (idx, self.all_face_neighbors_occupied(pos)))
            .collect();

        for (idx, interior) in updates {
            if interior {
                self.nodes[idx].child_pointer |= OctreeNode::INTERIOR_FLAG;
            } else {
                self.nodes[idx].child_pointer &= !OctreeNode::INTERIOR_FLAG;
            }
            self.mark_dirty(idx);
        }
    }

    fn collect_occupied_leaves(
        &self,
        idx: usize,
        origin: IVec3,
        size: i32,
        out: &mut Vec<(usize, IVec3)>,
    ) {
        let node = &self.nodes[idx];
        if node.is_leaf() {
            if node.color_data != 0 {
                let half = size / 2;
                out.push((idx, origin + IVec3::splat(half)));
            }
            return;
        }
        let child_mask = node.child_mask();
        let first_child = node.first_child_index();
        let child_half = size / 2;
        for slot in 0u8..8 {
            if child_mask & (1 << slot) == 0 {
                continue;
            }
            let child_offset =
                (child_mask & ((1 << slot) - 1)).count_ones() as usize;
            let child_idx = first_child + child_offset;
            let dx = if slot & 1 != 0 { child_half } else { 0 };
            let dy = if slot & 2 != 0 { child_half } else { 0 };
            let dz = if slot & 4 != 0 { child_half } else { 0 };
            self.collect_occupied_leaves(
                child_idx,
                origin + IVec3::new(dx, dy, dz),
                child_half,
                out,
            );
        }
    }
}
