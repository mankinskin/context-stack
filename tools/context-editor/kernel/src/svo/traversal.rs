use super::*;

impl VoxelWorld {
    /// World-space position of the SVO root's min corner.
    pub fn origin(&self) -> Vec3 {
        Vec3::ZERO
    }

    /// Total side length of the SVO root cube in world units (`2^max_depth`).
    pub fn world_size(&self) -> f32 {
        (1u32 << self.max_depth) as f32
    }

    /// Compute world-space positions for every node, returned as `[x, y, z, half_extent]`.
    pub fn compute_node_positions(&self) -> Vec<[f32; 4]> {
        let world_size = (1u32 << self.max_depth) as f32;
        let mut positions = vec![[0.0f32; 4]; self.nodes.len()];
        self.fill_positions(
            self.root_index as usize,
            0.0,
            0.0,
            0.0,
            world_size,
            &mut positions,
        );
        positions
    }

    fn fill_positions(
        &self,
        idx: usize,
        ox: f32,
        oy: f32,
        oz: f32,
        size: f32,
        out: &mut Vec<[f32; 4]>,
    ) {
        let half = size * 0.5;
        out[idx] = [ox + half, oy + half, oz + half, half];

        let node = &self.nodes[idx];
        let mask = node.child_mask();
        if mask == 0 {
            return;
        }
        let first_child = node.first_child_index();
        let child_size = half;
        for slot in 0..8usize {
            if mask & (1 << slot) == 0 {
                continue;
            }
            let cx = if slot & 1 != 0 { ox + half } else { ox };
            let cy = if slot & 2 != 0 { oy + half } else { oy };
            let cz = if slot & 4 != 0 { oz + half } else { oz };
            self.fill_positions(first_child + slot, cx, cy, cz, child_size, out);
        }
    }

    /// Traverse to the leaf node at the given position, returning its index.
    pub fn descend_to(
        &self,
        pos: IVec3,
    ) -> Option<usize> {
        if pos.x < 0 || pos.y < 0 || pos.z < 0 {
            return None;
        }
        let mut idx = self.root_index as usize;
        let mut size = 1u32 << self.max_depth;
        let (mut ox, mut oy, mut oz) = (0u32, 0u32, 0u32);

        for _ in 0..self.max_depth {
            let node = &self.nodes[idx];
            if node.is_leaf() {
                return Some(idx);
            }
            size >>= 1;
            let cx = (pos.x as u32).wrapping_sub(ox) >= size;
            let cy = (pos.y as u32).wrapping_sub(oy) >= size;
            let cz = (pos.z as u32).wrapping_sub(oz) >= size;
            if cx {
                ox += size;
            }
            if cy {
                oy += size;
            }
            if cz {
                oz += size;
            }
            let child_bit = child_index(cx, cy, cz);
            if node.child_mask() & (1 << child_bit) == 0 {
                return None;
            }
            idx = node.first_child_index() + child_bit;
        }
        Some(idx)
    }

    /// CPU-side ray-octree intersection.
    pub fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_dist: f32,
    ) -> Option<(Vec3, Vec3)> {
        let dir = direction.normalize();
        let step = 0.5f32;
        let mut t = 0.0f32;
        while t < max_dist {
            let p = origin + dir * t;
            let cell = p.floor().as_ivec3();
            if self
                .descend_to(cell)
                .map(|idx| self.nodes[idx].color_data != 0)
                .unwrap_or(false)
            {
                let eps = 0.5;
                let normal = Vec3::new(
                    self.density(cell + IVec3::X)
                        - self.density(cell - IVec3::X),
                    self.density(cell + IVec3::Y)
                        - self.density(cell - IVec3::Y),
                    self.density(cell + IVec3::Z)
                        - self.density(cell - IVec3::Z),
                ) * eps;
                let normal = if normal.length_squared() > 0.0 {
                    -normal.normalize()
                } else {
                    Vec3::Y
                };
                return Some((p, normal));
            }
            t += step;
        }
        None
    }

    fn density(
        &self,
        pos: IVec3,
    ) -> f32 {
        self.descend_to(pos)
            .map(|idx| if self.nodes[idx].color_data != 0 { 1.0 } else { 0.0 })
            .unwrap_or(0.0)
    }
}
