use super::{
    NodeType,
    DEFAULT_CUBE_HALF_EXTENT,
    DEFAULT_NODE_RADIUS,
};
use bevy::prelude::IVec3;

/// Shape of a node's voxel cluster.
#[derive(Clone, Debug)]
pub enum ClusterShape {
    Sphere { radius_voxels: u32 },
    Cube { half_extent_voxels: u32 },
    Custom(Vec<IVec3>),
}

impl ClusterShape {
    /// Generate the list of voxel offsets for this shape.
    pub fn voxel_offsets(&self) -> Vec<IVec3> {
        match self {
            ClusterShape::Sphere { radius_voxels } =>
                sphere_offsets(*radius_voxels as i32),
            ClusterShape::Cube { half_extent_voxels } =>
                cube_offsets(*half_extent_voxels as i32),
            ClusterShape::Custom(offsets) => offsets.clone(),
        }
    }
}

/// Generate sphere voxel offsets.
pub(crate) fn sphere_offsets(radius: i32) -> Vec<IVec3> {
    let mut offsets = Vec::new();
    let r2 = radius * radius;
    for x in -radius..=radius {
        for y in -radius..=radius {
            for z in -radius..=radius {
                if x * x + y * y + z * z <= r2 {
                    offsets.push(IVec3::new(x, y, z));
                }
            }
        }
    }
    offsets
}

/// Generate cube voxel offsets.
pub(crate) fn cube_offsets(half: i32) -> Vec<IVec3> {
    let mut offsets = Vec::new();
    for x in -half..=half {
        for y in -half..=half {
            for z in -half..=half {
                offsets.push(IVec3::new(x, y, z));
            }
        }
    }
    offsets
}

/// Get the cluster shape for a node type.
pub fn shape_for_type(node_type: NodeType) -> ClusterShape {
    match node_type {
        NodeType::Atom => ClusterShape::Sphere { radius_voxels: 2 },
        NodeType::Sequence => ClusterShape::Cube {
            half_extent_voxels: DEFAULT_CUBE_HALF_EXTENT,
        },
        NodeType::Hyperedge => ClusterShape::Sphere {
            radius_voxels: DEFAULT_NODE_RADIUS,
        },
        NodeType::Reference => ClusterShape::Cube {
            half_extent_voxels: 1,
        },
    }
}
