use super::*;

#[test]
fn prediction_buffer_ring() {
    let mut buffer = PredictionBuffer::new(3);
    for tick in 0..5 {
        buffer.push(PredictedState {
            tick,
            position: Vec3::new(tick as f32, 0.0, 0.0),
            velocity: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            input_applied: true,
        });
    }
    assert_eq!(buffer.len(), 3);
    assert!(buffer.find_tick(0).is_none());
    assert!(buffer.find_tick(1).is_none());
    assert!(buffer.find_tick(4).is_some());
}

#[test]
fn reconcile_agrees_within_threshold() {
    let snapshot = EntitySnapshot {
        tick: 10,
        position: Vec3::new(1.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };
    let predicted = PredictedState {
        tick: 10,
        position: Vec3::new(1.05, 0.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        input_applied: false,
    };
    let thresholds = RollbackThresholds::default();
    assert_eq!(
        reconcile(&snapshot, &predicted, &thresholds),
        ReconcileResult::Agree
    );
}

#[test]
fn reconcile_triggers_rollback() {
    let snapshot = EntitySnapshot {
        tick: 10,
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };
    let predicted = PredictedState {
        tick: 10,
        position: Vec3::new(5.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        input_applied: false,
    };
    let thresholds = RollbackThresholds::default();
    match reconcile(&snapshot, &predicted, &thresholds) {
        ReconcileResult::Rollback { position_error, .. } => {
            assert!((position_error - 5.0).abs() < 0.01);
        },
        ReconcileResult::Agree => panic!("should have rolled back"),
    }
}

#[test]
fn hermite_endpoints() {
    let prev = EntitySnapshot {
        tick: 0,
        position: Vec3::ZERO,
        velocity: Vec3::X,
        rotation: Quat::IDENTITY,
    };
    let next = EntitySnapshot {
        tick: 1,
        position: Vec3::new(10.0, 0.0, 0.0),
        velocity: Vec3::X,
        rotation: Quat::IDENTITY,
    };
    let state = HermiteState::from_snapshots(&prev, &next, 1.0);

    let start = HermiteState {
        t: 0.0,
        ..state.clone()
    };
    assert!((start.position() - Vec3::ZERO).length() < 0.01);

    let end = HermiteState {
        t: 1.0,
        ..state.clone()
    };
    assert!((end.position() - Vec3::new(10.0, 0.0, 0.0)).length() < 0.01);
}

#[test]
fn hermite_midpoint_is_smooth() {
    let prev = EntitySnapshot {
        tick: 0,
        position: Vec3::ZERO,
        velocity: Vec3::new(5.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
    };
    let next = EntitySnapshot {
        tick: 1,
        position: Vec3::new(10.0, 0.0, 0.0),
        velocity: Vec3::new(5.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
    };
    let mut state = HermiteState::from_snapshots(&prev, &next, 1.0);
    state.t = 0.5;
    let mid = state.position();
    assert!(
        mid.x > 3.0 && mid.x < 7.0,
        "midpoint x={} should be roughly 5",
        mid.x
    );
}

#[test]
fn rollback_reapplies_inputs() {
    let mut buffer = PredictionBuffer::new(16);
    for tick in 10..=12 {
        buffer.push(PredictedState {
            tick,
            position: Vec3::new(tick as f32, 0.0, 0.0),
            velocity: Vec3::new(1.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            input_applied: true,
        });
    }

    let snapshot = EntitySnapshot {
        tick: 9,
        position: Vec3::new(9.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };

    let corrected = apply_rollback(&snapshot, &buffer, 1.0);
    assert!(
        (corrected.x - 12.0).abs() < 0.01,
        "corrected.x={}",
        corrected.x
    );
}

#[test]
fn states_after_filters_correctly() {
    let mut buffer = PredictionBuffer::new(8);
    for tick in 5..=10 {
        buffer.push(PredictedState {
            tick,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            input_applied: false,
        });
    }
    let after = buffer.states_after(7);
    assert_eq!(after.len(), 3);
    assert_eq!(after[0].tick, 8);
    assert_eq!(after[2].tick, 10);
}
