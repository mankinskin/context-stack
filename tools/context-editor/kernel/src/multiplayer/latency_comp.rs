//! Multiplayer latency compensation — client-side prediction, rollback, and
//! Hermite SDF ghosting.
//!
//! ## Client-Side Prediction
//!
//! Player input is applied immediately on the client. A ring buffer of
//! predicted states is maintained so the client can reconcile with
//! authoritative server snapshots when they arrive.
//!
//! ## Rollback
//!
//! When a server snapshot arrives that disagrees with the predicted state,
//! the client rolls back to the server state and re-applies unacknowledged
//! inputs.
//!
//! ## Hermite SDF Ghosting
//!
//! Remote players are rendered with Hermite-interpolated SDF capsules to
//! produce smooth motion even under jitter.

use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Snapshot types
// ---------------------------------------------------------------------------

/// A compact server-authoritative snapshot for one entity.
#[derive(Clone, Debug)]
pub struct EntitySnapshot {
    /// Server tick this snapshot was taken at.
    pub tick: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
}

/// Ring buffer storing the last N predicted local states.
#[derive(Clone, Debug)]
pub struct PredictionBuffer {
    buf: Vec<PredictedState>,
    head: usize,
    capacity: usize,
}

/// A single predicted state entry.
#[derive(Clone, Debug)]
pub struct PredictedState {
    pub tick: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    pub input_applied: bool,
}

impl PredictionBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            head: 0,
            capacity,
        }
    }

    /// Push a new predicted state, overwriting the oldest if at capacity.
    pub fn push(
        &mut self,
        state: PredictedState,
    ) {
        if self.buf.len() < self.capacity {
            self.buf.push(state);
        } else {
            self.buf[self.head] = state;
        }
        self.head = (self.head + 1) % self.capacity;
    }

    /// Find the predicted state for a given tick.
    pub fn find_tick(
        &self,
        tick: u64,
    ) -> Option<&PredictedState> {
        self.buf.iter().find(|s| s.tick == tick)
    }

    /// Return all states with tick > `since` in tick order.
    pub fn states_after(
        &self,
        since: u64,
    ) -> Vec<&PredictedState> {
        let mut result: Vec<&PredictedState> =
            self.buf.iter().filter(|s| s.tick > since).collect();
        result.sort_by_key(|s| s.tick);
        result
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.buf.clear();
        self.head = 0;
    }
}

// ---------------------------------------------------------------------------
// Rollback engine
// ---------------------------------------------------------------------------

/// Result of comparing a server snapshot against predicted state.
#[derive(Debug, PartialEq)]
pub enum ReconcileResult {
    /// Predicted state is close enough — no correction needed.
    Agree,
    /// Predicted state diverged — rollback required.
    Rollback {
        position_error: f32,
        velocity_error: f32,
    },
}

/// Threshold for position/velocity mismatch that triggers rollback.
#[derive(Resource, Clone, Debug)]
pub struct RollbackThresholds {
    /// Maximum position error before rollback (world units).
    pub position: f32,
    /// Maximum velocity error before rollback (units/sec).
    pub velocity: f32,
}

impl Default for RollbackThresholds {
    fn default() -> Self {
        Self {
            position: 0.1,
            velocity: 0.5,
        }
    }
}

/// Compare a server snapshot against predicted state and decide whether to
/// roll back.
pub fn reconcile(
    snapshot: &EntitySnapshot,
    predicted: &PredictedState,
    thresholds: &RollbackThresholds,
) -> ReconcileResult {
    let pos_err = (snapshot.position - predicted.position).length();
    let vel_err = (snapshot.velocity - predicted.velocity).length();

    if pos_err > thresholds.position || vel_err > thresholds.velocity {
        ReconcileResult::Rollback {
            position_error: pos_err,
            velocity_error: vel_err,
        }
    } else {
        ReconcileResult::Agree
    }
}

/// Apply rollback: snap to server state and reapply unacknowledged inputs.
///
/// Returns the corrected position after re-simulation.
pub fn apply_rollback(
    server_snapshot: &EntitySnapshot,
    buffer: &PredictionBuffer,
    dt: f32,
) -> Vec3 {
    let mut pos = server_snapshot.position;
    let mut vel = server_snapshot.velocity;

    // Re-simulate all inputs after the server tick
    for state in buffer.states_after(server_snapshot.tick) {
        if state.input_applied {
            // Re-apply the input delta (simplified: use the stored velocity)
            vel = state.velocity;
        }
        pos += vel * dt;
    }

    pos
}

// ---------------------------------------------------------------------------
// Hermite interpolation for remote player ghosting
// ---------------------------------------------------------------------------

/// Hermite interpolation state for a remote entity.
#[derive(Clone, Debug)]
pub struct HermiteState {
    pub p0: Vec3,
    pub p1: Vec3,
    pub v0: Vec3,
    pub v1: Vec3,
    pub r0: Quat,
    pub r1: Quat,
    pub t: f32,
    pub duration: f32,
}

impl HermiteState {
    /// Create from two consecutive server snapshots.
    pub fn from_snapshots(
        prev: &EntitySnapshot,
        next: &EntitySnapshot,
        duration: f32,
    ) -> Self {
        Self {
            p0: prev.position,
            p1: next.position,
            v0: prev.velocity * duration,
            v1: next.velocity * duration,
            r0: prev.rotation,
            r1: next.rotation,
            t: 0.0,
            duration,
        }
    }

    /// Advance time by `dt` seconds.
    pub fn tick(
        &mut self,
        dt: f32,
    ) {
        self.t = (self.t + dt).min(self.duration);
    }

    /// Normalised parameter [0, 1].
    pub fn alpha(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            (self.t / self.duration).clamp(0.0, 1.0)
        }
    }

    /// Evaluate cubic Hermite position at current time.
    pub fn position(&self) -> Vec3 {
        let t = self.alpha();
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        self.p0 * h00 + self.v0 * h10 + self.p1 * h01 + self.v1 * h11
    }

    /// Evaluate rotation via spherical linear interpolation.
    pub fn rotation(&self) -> Quat {
        self.r0.slerp(self.r1, self.alpha())
    }

    /// Whether the interpolation has reached the end.
    pub fn is_complete(&self) -> bool {
        self.t >= self.duration
    }
}

// ---------------------------------------------------------------------------
// Bevy resources
// ---------------------------------------------------------------------------

/// Bevy resource holding local player prediction state.
#[derive(Resource)]
pub struct LocalPrediction {
    pub buffer: PredictionBuffer,
    pub current_tick: u64,
    pub last_server_tick: u64,
}

impl Default for LocalPrediction {
    fn default() -> Self {
        Self {
            buffer: PredictionBuffer::new(128),
            current_tick: 0,
            last_server_tick: 0,
        }
    }
}

/// Bevy resource holding Hermite states for all remote players, keyed by
/// entity ID.
#[derive(Resource, Default)]
pub struct RemoteGhosts {
    pub ghosts: Vec<(u64, HermiteState)>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers latency compensation resources.
pub struct LatencyCompPlugin;

impl Plugin for LatencyCompPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<RollbackThresholds>();
        app.init_resource::<LocalPrediction>();
        app.init_resource::<RemoteGhosts>();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prediction_buffer_ring() {
        let mut buf = PredictionBuffer::new(3);
        for i in 0..5 {
            buf.push(PredictedState {
                tick: i,
                position: Vec3::new(i as f32, 0.0, 0.0),
                velocity: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                input_applied: true,
            });
        }
        // Capacity is 3 — only ticks 2, 3, 4 should remain
        assert_eq!(buf.len(), 3);
        assert!(buf.find_tick(0).is_none());
        assert!(buf.find_tick(1).is_none());
        assert!(buf.find_tick(4).is_some());
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
        let thresholds = RollbackThresholds::default(); // 0.1 pos
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
            position: Vec3::new(5.0, 0.0, 0.0), // large error
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

        // At t=0, position should be p0
        let h0 = HermiteState {
            t: 0.0,
            ..state.clone()
        };
        assert!((h0.position() - Vec3::ZERO).length() < 0.01);

        // At t=duration, position should be p1
        let h1 = HermiteState {
            t: 1.0,
            ..state.clone()
        };
        assert!((h1.position() - Vec3::new(10.0, 0.0, 0.0)).length() < 0.01);
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
        // Midpoint should be roughly halfway
        assert!(
            mid.x > 3.0 && mid.x < 7.0,
            "midpoint x={} should be roughly 5",
            mid.x
        );
    }

    #[test]
    fn rollback_reapplies_inputs() {
        let mut buf = PredictionBuffer::new(16);
        // Simulate ticks 10, 11, 12 with velocity 1.0 in X
        for tick in 10..=12 {
            buf.push(PredictedState {
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

        // dt = 1.0 per tick, 3 ticks to reapply (10, 11, 12)
        let corrected = apply_rollback(&snapshot, &buf, 1.0);
        // Should have moved 3 units from 9.0 → 12.0
        assert!(
            (corrected.x - 12.0).abs() < 0.01,
            "corrected.x={}",
            corrected.x
        );
    }

    #[test]
    fn states_after_filters_correctly() {
        let mut buf = PredictionBuffer::new(8);
        for tick in 5..=10 {
            buf.push(PredictedState {
                tick,
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                input_applied: false,
            });
        }
        let after = buf.states_after(7);
        assert_eq!(after.len(), 3); // ticks 8, 9, 10
        assert_eq!(after[0].tick, 8);
        assert_eq!(after[2].tick, 10);
    }
}
