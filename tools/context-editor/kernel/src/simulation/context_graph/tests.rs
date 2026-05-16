use super::*;

#[test]
fn sphere_offsets_center() {
    let offsets = sphere_offsets(1);
    assert!(offsets.contains(&IVec3::ZERO));
}

#[test]
fn sphere_offsets_count() {
    let offsets = sphere_offsets(1);
    assert_eq!(offsets.len(), 7);
}

#[test]
fn cube_offsets_count() {
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
    assert!(f.x < 0.0);
}

#[test]
fn attraction_force_pulls_together() {
    let f = attraction_force(Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0));
    assert!(f.x > 0.0);
}

#[test]
fn layout_step_moves_nodes() {
    let mut g = GraphData::default();
    g.add_node(NodeId(1), NodeType::Atom, "a".into());
    g.add_node(NodeId(2), NodeType::Atom, "b".into());
    g.nodes.get_mut(&NodeId(1)).unwrap().position = Vec3::new(-5.0, 0.0, 0.0);
    g.nodes.get_mut(&NodeId(2)).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    let pos1_before = g.nodes[&NodeId(1)].position;
    layout_step(&mut g, 0.016);
    let pos1_after = g.nodes[&NodeId(1)].position;
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
    assert_eq!(points.len(), 6);
    assert_eq!(points[0], IVec3::ZERO);
    assert_eq!(points[5], IVec3::new(5, 0, 0));
}

#[test]
fn voxel_line_diagonal() {
    let points = voxel_line(IVec3::ZERO, IVec3::new(3, 3, 3));
    assert!(points.len() >= 4);
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
