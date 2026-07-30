use super::*;
use std::collections::HashMap;

#[test]
fn ticket_state_roundtrip() {
    for state in [
        TicketState::Open,
        TicketState::Planned,
        TicketState::InImplementation,
        TicketState::InReview,
        TicketState::Done,
        TicketState::Cancelled,
    ] {
        assert_eq!(TicketState::from_str(state.as_str()), state);
    }
}

#[test]
fn ticket_state_terminal() {
    assert!(TicketState::Done.is_terminal());
    assert!(TicketState::Cancelled.is_terminal());
    assert!(!TicketState::Open.is_terminal());
    assert!(!TicketState::InImplementation.is_terminal());
}

#[test]
fn ticket_state_tint_distinct() {
    let new_tint = TicketState::Open.tint_color();
    let done_tint = TicketState::Done.tint_color();
    assert_ne!(new_tint, done_tint);
}

#[test]
fn ticket_state_unknown_defaults_new() {
    assert_eq!(TicketState::from_str("bogus"), TicketState::Open);
}

#[test]
fn priority_roughness_order() {
    assert!(
        Priority::Critical.glass_roughness() < Priority::High.glass_roughness()
    );
    assert!(
        Priority::High.glass_roughness() < Priority::Medium.glass_roughness()
    );
    assert!(
        Priority::Medium.glass_roughness() < Priority::Low.glass_roughness()
    );
}

#[test]
fn priority_from_str() {
    assert_eq!(Priority::from_str("critical"), Priority::Critical);
    assert_eq!(Priority::from_str("high"), Priority::High);
    assert_eq!(Priority::from_str("unknown"), Priority::None);
}

#[test]
fn edge_kind_colors_distinct() {
    assert_ne!(
        EdgeKind::DependsOn.edge_color(),
        EdgeKind::Blocks.edge_color()
    );
    assert_ne!(
        EdgeKind::Blocks.edge_color(),
        EdgeKind::RelatedTo.edge_color()
    );
}

#[test]
fn edge_kind_metallic() {
    assert!(EdgeKind::Blocks.is_metallic());
    assert!(!EdgeKind::DependsOn.is_metallic());
    assert!(!EdgeKind::RelatedTo.is_metallic());
}

#[test]
fn store_upsert_and_get() {
    let mut store = TicketStore::default();
    store.upsert(TicketData {
        id: "abc".into(),
        title: "Test ticket".into(),
        state: TicketState::Open,
        priority: Priority::High,
        ticket_type: "bug".into(),
        description: "A test".into(),
    });
    assert_eq!(store.ticket_count(), 1);
    assert_eq!(store.get("abc").unwrap().title, "Test ticket");
    assert!(store.dirty);
}

#[test]
fn store_upsert_overwrites() {
    let mut store = TicketStore::default();
    store.upsert(TicketData {
        id: "abc".into(),
        title: "v1".into(),
        state: TicketState::Open,
        priority: Priority::None,
        ticket_type: "task".into(),
        description: "".into(),
    });
    store.upsert(TicketData {
        id: "abc".into(),
        title: "v2".into(),
        state: TicketState::Done,
        priority: Priority::High,
        ticket_type: "task".into(),
        description: "updated".into(),
    });
    assert_eq!(store.ticket_count(), 1);
    assert_eq!(store.get("abc").unwrap().title, "v2");
    assert_eq!(store.get("abc").unwrap().state, TicketState::Done);
}

#[test]
fn store_remove() {
    let mut store = TicketStore::default();
    store.upsert(TicketData {
        id: "abc".into(),
        title: "t".into(),
        state: TicketState::Open,
        priority: Priority::None,
        ticket_type: "task".into(),
        description: "".into(),
    });
    let removed = store.remove("abc");
    assert!(removed.is_some());
    assert_eq!(store.ticket_count(), 0);
    assert!(store.remove("abc").is_none());
}

#[test]
fn store_edges_and_neighbors() {
    let mut store = TicketStore::default();
    store.upsert(TicketData {
        id: "a".into(),
        title: "A".into(),
        state: TicketState::Open,
        priority: Priority::None,
        ticket_type: "task".into(),
        description: "".into(),
    });
    store.upsert(TicketData {
        id: "b".into(),
        title: "B".into(),
        state: TicketState::Open,
        priority: Priority::None,
        ticket_type: "task".into(),
        description: "".into(),
    });
    store.add_edge(TicketEdge {
        from: "a".into(),
        to: "b".into(),
        kind: EdgeKind::DependsOn,
    });
    assert_eq!(store.edge_count(), 1);
    let neighbors = store.neighbors("a");
    assert_eq!(neighbors, vec!["b"]);
    let neighbors_b = store.neighbors("b");
    assert_eq!(neighbors_b, vec!["a"]);
}

#[test]
fn store_by_state() {
    let mut store = TicketStore::default();
    store.upsert(TicketData {
        id: "a".into(),
        title: "A".into(),
        state: TicketState::Open,
        priority: Priority::None,
        ticket_type: "task".into(),
        description: "".into(),
    });
    store.upsert(TicketData {
        id: "b".into(),
        title: "B".into(),
        state: TicketState::Done,
        priority: Priority::None,
        ticket_type: "task".into(),
        description: "".into(),
    });
    store.upsert(TicketData {
        id: "c".into(),
        title: "C".into(),
        state: TicketState::Open,
        priority: Priority::None,
        ticket_type: "task".into(),
        description: "".into(),
    });
    assert_eq!(store.by_state(TicketState::Open).len(), 2);
    assert_eq!(store.by_state(TicketState::Done).len(), 1);
    assert_eq!(store.by_state(TicketState::Cancelled).len(), 0);
}

#[test]
fn store_state_counts() {
    let mut store = TicketStore::default();
    store.upsert(TicketData {
        id: "a".into(),
        title: "A".into(),
        state: TicketState::Open,
        priority: Priority::None,
        ticket_type: "t".into(),
        description: "".into(),
    });
    store.upsert(TicketData {
        id: "b".into(),
        title: "B".into(),
        state: TicketState::Open,
        priority: Priority::None,
        ticket_type: "t".into(),
        description: "".into(),
    });
    store.upsert(TicketData {
        id: "c".into(),
        title: "C".into(),
        state: TicketState::Done,
        priority: Priority::None,
        ticket_type: "t".into(),
        description: "".into(),
    });
    let counts = store.state_counts();
    assert_eq!(*counts.get(&TicketState::Open).unwrap(), 2);
    assert_eq!(*counts.get(&TicketState::Done).unwrap(), 1);
}

#[test]
fn grid_positions_no_overlap() {
    let ids = vec!["a", "b", "c", "d", "e", "f"];
    let positions = grid_positions(&ids);
    assert_eq!(positions.len(), 6);
    let pos_list: Vec<Vec3> = positions.values().cloned().collect();
    for i in 0..pos_list.len() {
        for j in (i + 1)..pos_list.len() {
            assert_ne!(pos_list[i], pos_list[j]);
        }
    }
}

#[test]
fn grid_positions_respects_columns() {
    let ids: Vec<&str> = (0..10)
        .map(|i| match i {
            0 => "a",
            1 => "b",
            2 => "c",
            3 => "d",
            4 => "e",
            5 => "f",
            6 => "g",
            7 => "h",
            8 => "i",
            _ => "j",
        })
        .collect();
    let positions = grid_positions(&ids);
    let first_pos = positions["a"];
    let sixth_pos = positions["f"];
    assert_eq!(first_pos.z, 0.0);
    assert_eq!(sixth_pos.z, GRID_SPACING);
}

#[test]
fn grid_positions_empty() {
    let ids: Vec<&str> = vec![];
    let positions = grid_positions(&ids);
    assert!(positions.is_empty());
}

#[test]
fn force_directed_step_repels() {
    let mut positions = HashMap::new();
    positions.insert("a".to_string(), Vec3::new(0.0, 0.0, 0.0));
    positions.insert("b".to_string(), Vec3::new(1.0, 0.0, 0.0));

    let mut velocities = HashMap::new();
    velocities.insert("a".to_string(), Vec3::ZERO);
    velocities.insert("b".to_string(), Vec3::ZERO);

    force_directed_step(&mut positions, &mut velocities, &[], 1.0);

    assert!(velocities["a"].x < 0.0, "a should be pushed left");
    assert!(velocities["b"].x > 0.0, "b should be pushed right");
}

#[test]
fn force_directed_step_attracts_along_edge() {
    let mut positions = HashMap::new();
    positions.insert("a".to_string(), Vec3::new(0.0, 0.0, 0.0));
    positions.insert("b".to_string(), Vec3::new(100.0, 0.0, 0.0));

    let mut velocities = HashMap::new();
    velocities.insert("a".to_string(), Vec3::ZERO);
    velocities.insert("b".to_string(), Vec3::ZERO);

    let edges = vec![TicketEdge {
        from: "a".to_string(),
        to: "b".to_string(),
        kind: EdgeKind::DependsOn,
    }];

    force_directed_step(&mut positions, &mut velocities, &edges, 1.0);

    assert!(velocities["a"].x > 0.0, "a should be attracted toward b");
}

#[test]
fn force_directed_velocity_clamped() {
    let mut positions = HashMap::new();
    positions.insert("a".to_string(), Vec3::ZERO);
    positions.insert("b".to_string(), Vec3::new(0.1, 0.0, 0.0));

    let mut velocities = HashMap::new();
    velocities.insert("a".to_string(), Vec3::ZERO);
    velocities.insert("b".to_string(), Vec3::ZERO);

    force_directed_step(&mut positions, &mut velocities, &[], 1.0);

    assert!(velocities["a"].length() <= MAX_VELOCITY + 0.001);
    assert!(velocities["b"].length() <= MAX_VELOCITY + 0.001);
}

#[test]
fn voxel_line_single_point() {
    let line = voxel_line_3d(Vec3::ZERO, Vec3::ZERO);
    assert_eq!(line.len(), 1);
    assert_eq!(line[0], (0, 0, 0));
}

#[test]
fn voxel_line_horizontal() {
    let line =
        voxel_line_3d(Vec3::new(0.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
    assert!(line.len() >= 5);
    assert_eq!(line[0], (0, 0, 0));
    assert_eq!(*line.last().unwrap(), (5, 0, 0));
    for &(_, y, z) in &line {
        assert_eq!(y, 0);
        assert_eq!(z, 0);
    }
}

#[test]
fn voxel_line_diagonal() {
    let line =
        voxel_line_3d(Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 3.0, 3.0));
    assert!(!line.is_empty());
    assert_eq!(line[0], (0, 0, 0));
    assert_eq!(*line.last().unwrap(), (3, 3, 3));
}

#[test]
fn voxel_line_no_duplicates() {
    let line =
        voxel_line_3d(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 5.0, 3.0));
    for i in 1..line.len() {
        assert_ne!(line[i], line[i - 1], "Duplicate entry at index {}", i);
    }
}

#[test]
fn selection_default_none() {
    let sel = TicketSelection::default();
    assert!(sel.selected_id.is_none());
}
