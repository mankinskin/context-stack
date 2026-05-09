//! Context graph 3D: hypergraph nodes as voxel clusters generating splats.
//!
//! Each context-engine hypergraph node is placed as a small voxel cluster in
//! the SVO. The splat pipeline converts these to soft volumetric shapes. Edges
//! are voxel lines between nodes. Force-directed layout positions the graph.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::svo::{
    VoxelMaterial,
    VoxelWorld,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default node cluster radius (in voxels).
pub const DEFAULT_NODE_RADIUS: u32 = 3;

/// Default cube half-extent (in voxels).
pub const DEFAULT_CUBE_HALF_EXTENT: u32 = 2;

/// Force-directed layout: repulsion constant.
pub const REPULSION_K: f32 = 500.0;

/// Force-directed layout: attraction constant.
pub const ATTRACTION_K: f32 = 0.01;

/// Force-directed layout: ideal edge length.
pub const IDEAL_EDGE_LENGTH: f32 = 10.0;

/// Maximum force magnitude (prevents instability).
pub const MAX_FORCE: f32 = 50.0;

/// Layout damping factor per frame.
pub const DAMPING: f32 = 0.9;

/// Camera zoom lerp speed.
pub const ZOOM_LERP_SPEED: f32 = 5.0;

/// Label offset above node center.
pub const LABEL_Y_OFFSET: f32 = 3.0;

// ---------------------------------------------------------------------------
// Node/edge types
// ---------------------------------------------------------------------------

/// Unique identifier for a graph node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

/// Type of hypergraph node, determines shape and color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeType {
    /// Atom node (single token/char).
    Atom,
    /// Sequence node (ordered chain).
    Sequence,
    /// Hyperedge collector.
    Hyperedge,
    /// External reference (file, URL).
    Reference,
}

/// Edge types with distinct visual styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeType {
    /// Sequential ordering between atoms/sequences.
    Sequence,
    /// Dependency / relationship link.
    Dependency,
    /// Hyperedge membership.
    Hyperedge,
}

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
fn sphere_offsets(radius: i32) -> Vec<IVec3> {
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
fn cube_offsets(half: i32) -> Vec<IVec3> {
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

/// Get the material for a node type.
pub fn material_for_type(node_type: NodeType) -> VoxelMaterial {
    match node_type {
        NodeType::Atom => VoxelMaterial::new(100, 180, 255, 8), // light blue, glossy
        NodeType::Sequence => VoxelMaterial::new(80, 220, 120, 12), // green
        NodeType::Hyperedge => VoxelMaterial::new(255, 140, 60, 6), // orange, glossy
        NodeType::Reference => VoxelMaterial::new(200, 200, 210, 16), // silver
    }
}

/// Get the material for an edge type.
pub fn material_for_edge(edge_type: EdgeType) -> VoxelMaterial {
    match edge_type {
        EdgeType::Sequence => VoxelMaterial::new(160, 160, 170, 20), // muted grey
        EdgeType::Dependency =>
            VoxelMaterial::new_metallic(180, 190, 200, 10, true), // metallic
        EdgeType::Hyperedge => VoxelMaterial::new(255, 200, 80, 4), // bright gold
    }
}

// ---------------------------------------------------------------------------
// Graph data
// ---------------------------------------------------------------------------

/// A hypergraph node with layout position.
#[derive(Clone, Debug)]
pub struct HyperNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub label: String,
    pub position: Vec3,
    pub velocity: Vec3,
}

/// A directed edge between two nodes.
#[derive(Clone, Debug)]
pub struct HyperEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: EdgeType,
}

/// Resource holding the full graph state.
#[derive(Resource, Default)]
pub struct GraphData {
    pub nodes: HashMap<NodeId, HyperNode>,
    pub edges: Vec<HyperEdge>,
    /// Whether the graph has been modified since last voxel sync.
    pub dirty: bool,
}

impl GraphData {
    pub fn add_node(
        &mut self,
        id: NodeId,
        node_type: NodeType,
        label: String,
    ) {
        // Place new nodes at a pseudo-random offset to avoid overlap
        let hash = id.0.wrapping_mul(2654435761) as f32 / u64::MAX as f32;
        let pos = Vec3::new(
            (hash * 100.0) - 50.0,
            ((hash * 7.3).fract() * 100.0) - 50.0,
            ((hash * 13.7).fract() * 100.0) - 50.0,
        );
        self.nodes.insert(
            id,
            HyperNode {
                id,
                node_type,
                label,
                position: pos,
                velocity: Vec3::ZERO,
            },
        );
        self.dirty = true;
    }

    pub fn remove_node(
        &mut self,
        id: NodeId,
    ) {
        self.nodes.remove(&id);
        self.edges.retain(|e| e.from != id && e.to != id);
        self.dirty = true;
    }

    pub fn add_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        edge_type: EdgeType,
    ) {
        self.edges.push(HyperEdge {
            from,
            to,
            edge_type,
        });
        self.dirty = true;
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

// ---------------------------------------------------------------------------
// 3D node component
// ---------------------------------------------------------------------------

/// Bevy component for a graph node entity in the 3D scene.
#[derive(Component)]
pub struct GraphNode3D {
    pub node_id: NodeId,
    pub node_type: NodeType,
    pub cluster_shape: ClusterShape,
}

/// Component for a node label (billboarded text above node).
#[derive(Component)]
pub struct GraphNodeLabel {
    pub node_id: NodeId,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Selection & camera
// ---------------------------------------------------------------------------

/// Resource tracking the currently selected node.
#[derive(Resource, Default)]
pub struct GraphSelection {
    pub selected_node: Option<NodeId>,
    pub zoom_target: Option<Vec3>,
}

// ---------------------------------------------------------------------------
// Force-directed layout
// ---------------------------------------------------------------------------

/// Compute repulsion force between two nodes.
pub fn repulsion_force(
    pos_a: Vec3,
    pos_b: Vec3,
) -> Vec3 {
    let diff = pos_a - pos_b;
    let dist_sq = diff.length_squared().max(0.01);
    let dir = diff.normalize_or_zero();
    dir * REPULSION_K / dist_sq
}

/// Compute attraction force along an edge.
pub fn attraction_force(
    pos_from: Vec3,
    pos_to: Vec3,
) -> Vec3 {
    let diff = pos_to - pos_from;
    let dist = diff.length();
    let displacement = dist - IDEAL_EDGE_LENGTH;
    diff.normalize_or_zero() * ATTRACTION_K * displacement
}

/// Run one iteration of force-directed layout on the graph data.
pub fn layout_step(
    graph: &mut GraphData,
    dt: f32,
) {
    let ids: Vec<NodeId> = graph.nodes.keys().copied().collect();
    let positions: HashMap<NodeId, Vec3> = graph
        .nodes
        .iter()
        .map(|(id, n)| (*id, n.position))
        .collect();

    // Repulsion between all pairs
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let pa = positions[&ids[i]];
            let pb = positions[&ids[j]];
            let force = repulsion_force(pa, pb);
            let clamped = clamp_force(force);
            if let Some(na) = graph.nodes.get_mut(&ids[i]) {
                na.velocity += clamped * dt;
            }
            if let Some(nb) = graph.nodes.get_mut(&ids[j]) {
                nb.velocity -= clamped * dt;
            }
        }
    }

    // Attraction along edges
    for edge in &graph.edges {
        if let (Some(&pa), Some(&pb)) =
            (positions.get(&edge.from), positions.get(&edge.to))
        {
            let force = attraction_force(pa, pb);
            let clamped = clamp_force(force);
            if let Some(n) = graph.nodes.get_mut(&edge.from) {
                n.velocity += clamped * dt;
            }
            if let Some(n) = graph.nodes.get_mut(&edge.to) {
                n.velocity -= clamped * dt;
            }
        }
    }

    // Apply velocity and damping
    for node in graph.nodes.values_mut() {
        node.position += node.velocity * dt;
        node.velocity *= DAMPING;
    }

    graph.dirty = true;
}

fn clamp_force(f: Vec3) -> Vec3 {
    let len = f.length();
    if len > MAX_FORCE {
        f * (MAX_FORCE / len)
    } else {
        f
    }
}

// ---------------------------------------------------------------------------
// Voxel line rasterization
// ---------------------------------------------------------------------------

/// Rasterize a 3D line between two points into voxel offsets (Bresenham-like).
pub fn voxel_line(
    from: IVec3,
    to: IVec3,
) -> Vec<IVec3> {
    let mut points = Vec::new();
    let diff = to - from;
    let steps = diff.x.abs().max(diff.y.abs()).max(diff.z.abs()).max(1);

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let p = IVec3::new(
            from.x + (diff.x as f32 * t).round() as i32,
            from.y + (diff.y as f32 * t).round() as i32,
            from.z + (diff.z as f32 * t).round() as i32,
        );
        if points.last() != Some(&p) {
            points.push(p);
        }
    }

    points
}

/// Convert a world Vec3 to SVO integer coordinates.
pub fn world_to_svo(pos: Vec3) -> IVec3 {
    IVec3::new(
        pos.x.round() as i32,
        pos.y.round() as i32,
        pos.z.round() as i32,
    )
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// System: run force-directed layout iteration.
fn layout_system(
    time: Res<Time>,
    mut graph: ResMut<GraphData>,
) {
    if graph.node_count() < 2 {
        return;
    }
    layout_step(&mut graph, time.delta_secs());
}

/// System: smooth camera zoom toward selection target.
fn zoom_camera_system(
    time: Res<Time>,
    selection: Res<GraphSelection>,
    mut camera_q: Query<&mut Transform, With<Camera3d>>,
) {
    if let Some(target) = selection.zoom_target {
        if let Ok(mut cam_tf) = camera_q.single_mut() {
            let dir = (target - cam_tf.translation).normalize_or_zero();
            let dist = cam_tf.translation.distance(target);
            if dist > 1.0 {
                cam_tf.translation +=
                    dir * dist * ZOOM_LERP_SPEED * time.delta_secs();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ContextGraph3DPlugin;

impl Plugin for ContextGraph3DPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<GraphData>();
        app.init_resource::<GraphSelection>();
        app.add_systems(Update, (layout_system, zoom_camera_system));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_offsets_center() {
        let offsets = sphere_offsets(1);
        assert!(offsets.contains(&IVec3::ZERO));
    }

    #[test]
    fn sphere_offsets_count() {
        // Radius 1 sphere: 7 voxels (center + 6 face neighbors)
        let offsets = sphere_offsets(1);
        assert_eq!(offsets.len(), 7);
    }

    #[test]
    fn cube_offsets_count() {
        // Half-extent 1: 3x3x3 = 27
        let offsets = cube_offsets(1);
        assert_eq!(offsets.len(), 27);
    }

    #[test]
    fn shape_for_type_atom() {
        let shape = shape_for_type(NodeType::Atom);
        assert!(matches!(shape, ClusterShape::Sphere { radius_voxels: 2 }));
    }

    #[test]
    fn shape_for_type_sequence() {
        let shape = shape_for_type(NodeType::Sequence);
        assert!(matches!(shape, ClusterShape::Cube { .. }));
    }

    #[test]
    fn cluster_shape_voxel_offsets_nonempty() {
        let shape = ClusterShape::Sphere { radius_voxels: 2 };
        let offsets = shape.voxel_offsets();
        assert!(!offsets.is_empty());
        assert!(offsets.contains(&IVec3::ZERO));
    }

    #[test]
    fn material_for_type_distinct() {
        let atom_mat = material_for_type(NodeType::Atom);
        let hyper_mat = material_for_type(NodeType::Hyperedge);
        assert_ne!(atom_mat.pack(), hyper_mat.pack());
    }

    #[test]
    fn material_for_edge_metallic() {
        let dep_mat = material_for_edge(EdgeType::Dependency);
        assert!(dep_mat.metallic);
    }

    #[test]
    fn graph_data_add_remove_node() {
        let mut g = GraphData::default();
        g.add_node(NodeId(1), NodeType::Atom, "a".into());
        assert_eq!(g.node_count(), 1);
        g.remove_node(NodeId(1));
        assert_eq!(g.node_count(), 0);
    }

    #[test]
    fn graph_data_add_edge() {
        let mut g = GraphData::default();
        g.add_node(NodeId(1), NodeType::Atom, "a".into());
        g.add_node(NodeId(2), NodeType::Sequence, "ab".into());
        g.add_edge(NodeId(1), NodeId(2), EdgeType::Sequence);
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn graph_remove_node_removes_edges() {
        let mut g = GraphData::default();
        g.add_node(NodeId(1), NodeType::Atom, "a".into());
        g.add_node(NodeId(2), NodeType::Atom, "b".into());
        g.add_edge(NodeId(1), NodeId(2), EdgeType::Dependency);
        g.remove_node(NodeId(1));
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn graph_dirty_on_mutation() {
        let mut g = GraphData::default();
        g.dirty = false;
        g.add_node(NodeId(1), NodeType::Atom, "test".into());
        assert!(g.dirty);
    }

    #[test]
    fn repulsion_force_pushes_apart() {
        let f = repulsion_force(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        assert!(f.x < 0.0); // pushes ZERO away from (1,0,0)
    }

    #[test]
    fn attraction_force_pulls_together() {
        let f = attraction_force(Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0));
        assert!(f.x > 0.0); // pulls toward (20,0,0) if dist > ideal
    }

    #[test]
    fn layout_step_moves_nodes() {
        let mut g = GraphData::default();
        g.add_node(NodeId(1), NodeType::Atom, "a".into());
        g.add_node(NodeId(2), NodeType::Atom, "b".into());
        // Force nodes to known distinct positions
        g.nodes.get_mut(&NodeId(1)).unwrap().position =
            Vec3::new(-5.0, 0.0, 0.0);
        g.nodes.get_mut(&NodeId(2)).unwrap().position =
            Vec3::new(5.0, 0.0, 0.0);
        let pos1_before = g.nodes[&NodeId(1)].position;
        layout_step(&mut g, 0.016);
        let pos1_after = g.nodes[&NodeId(1)].position;
        // Nodes should have moved due to repulsion
        assert_ne!(pos1_before, pos1_after);
    }

    #[test]
    fn voxel_line_same_point() {
        let points = voxel_line(IVec3::ZERO, IVec3::ZERO);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0], IVec3::ZERO);
    }

    #[test]
    fn voxel_line_axis_aligned() {
        let points = voxel_line(IVec3::ZERO, IVec3::new(5, 0, 0));
        assert_eq!(points.len(), 6); // 0..=5
        assert_eq!(points[0], IVec3::ZERO);
        assert_eq!(points[5], IVec3::new(5, 0, 0));
    }

    #[test]
    fn voxel_line_diagonal() {
        let points = voxel_line(IVec3::ZERO, IVec3::new(3, 3, 3));
        assert!(points.len() >= 4); // at least the 4 corner-aligned steps
        assert_eq!(*points.first().unwrap(), IVec3::ZERO);
        assert_eq!(*points.last().unwrap(), IVec3::new(3, 3, 3));
    }

    #[test]
    fn world_to_svo_rounding() {
        assert_eq!(
            world_to_svo(Vec3::new(1.4, 2.6, -0.5)),
            IVec3::new(1, 3, -1)
        );
        assert_eq!(world_to_svo(Vec3::new(0.5, 0.5, 0.5)), IVec3::new(1, 1, 1));
    }

    #[test]
    fn clamp_force_under_limit() {
        let f = clamp_force(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(f, Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn clamp_force_over_limit() {
        let f = clamp_force(Vec3::new(100.0, 0.0, 0.0));
        assert!(f.length() <= MAX_FORCE + 0.01);
    }

    #[test]
    fn graph_selection_default() {
        let sel = GraphSelection::default();
        assert!(sel.selected_node.is_none());
        assert!(sel.zoom_target.is_none());
    }
}
