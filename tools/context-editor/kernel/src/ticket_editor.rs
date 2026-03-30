//! Ticket Editor: Tickets as Glass Panels in Voxel-Splatted World.
//!
//! Each ticket from the ticket-api is rendered as an interactive 3D glass SDF
//! panel displaying ticket fields. Dependency edges are drawn as voxel lines
//! that participate in splat generation, creating soft glowing connections.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::multiplayer_backend::PlayerIdentity;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default panel half-width (world units).
pub const PANEL_HALF_WIDTH: f32 = 1.5;

/// Default panel half-height (world units).
pub const PANEL_HALF_HEIGHT: f32 = 1.0;

/// Corner radius for ticket panels.
pub const PANEL_CORNER_RADIUS: f32 = 0.1;

/// Spacing between panels in grid layout.
pub const GRID_SPACING: f32 = 4.0;

/// Columns in grid layout.
pub const GRID_COLUMNS: usize = 5;

/// Spring constant for force-directed layout.
pub const SPRING_K: f32 = 0.05;

/// Repulsion constant for force-directed layout.
pub const REPULSION_K: f32 = 50.0;

/// Damping factor for force-directed velocity.
pub const LAYOUT_DAMPING: f32 = 0.85;

/// Maximum velocity magnitude.
pub const MAX_VELOCITY: f32 = 2.0;

/// Minimum distance for repulsion calculation (avoid division by zero).
pub const MIN_REPULSION_DIST: f32 = 0.5;

// ---------------------------------------------------------------------------
// Ticket types (mirrors ticket-api)
// ---------------------------------------------------------------------------

/// Ticket lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TicketState {
    New,
    InRefinement,
    Ready,
    InImplementation,
    InReview,
    InValidation,
    Done,
    Cancelled,
}

impl TicketState {
    pub fn from_str(s: &str) -> Self {
        match s {
            "new" => Self::New,
            "in-refinement" => Self::InRefinement,
            "ready" => Self::Ready,
            "in-implementation" => Self::InImplementation,
            "in-review" => Self::InReview,
            "in-validation" => Self::InValidation,
            "done" => Self::Done,
            "cancelled" => Self::Cancelled,
            _ => Self::New,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::New => "new",
            Self::InRefinement => "in-refinement",
            Self::Ready => "ready",
            Self::InImplementation => "in-implementation",
            Self::InReview => "in-review",
            Self::InValidation => "in-validation",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }

    /// Glass tint color for this state.
    pub fn tint_color(&self) -> (f32, f32, f32) {
        match self {
            Self::New => (0.3, 0.5, 1.0),               // Blue
            Self::InRefinement => (0.4, 0.6, 1.0),      // Light blue
            Self::Ready => (0.3, 0.8, 0.5),             // Teal-green
            Self::InImplementation => (1.0, 0.85, 0.2), // Yellow
            Self::InReview => (1.0, 0.6, 0.2),          // Orange
            Self::InValidation => (0.9, 0.5, 0.9),      // Purple
            Self::Done => (0.2, 0.9, 0.3),              // Green
            Self::Cancelled => (0.5, 0.5, 0.5),         // Gray
        }
    }
}

/// Ticket priority level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl Priority {
    pub fn from_str(s: &str) -> Self {
        match s {
            "critical" => Self::Critical,
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => Self::None,
        }
    }

    /// Glass roughness for this priority (clear = urgent, frosted = low).
    pub fn glass_roughness(&self) -> f32 {
        match self {
            Self::Critical => 0.0,
            Self::High => 0.15,
            Self::Medium => 0.4,
            Self::Low => 0.7,
            Self::None => 0.5,
        }
    }
}

/// Kind of dependency edge between tickets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    DependsOn,
    Blocks,
    RelatedTo,
}

impl EdgeKind {
    /// Packed RGBA voxel color for this edge type.
    pub fn edge_color(&self) -> u32 {
        match self {
            Self::DependsOn => 0xFF8844FF,   // Orange — dependency
            Self::Blocks => 0xFF4444FF,       // Red — blocker
            Self::RelatedTo => 0x88AAFFFF,    // Light blue — informational
        }
    }

    /// Whether edge voxels should use metallic PBR (bright specular highlights).
    pub fn is_metallic(&self) -> bool {
        matches!(self, Self::Blocks)
    }
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A ticket record mirroring the ticket-api schema.
#[derive(Clone, Debug)]
pub struct TicketData {
    pub id: String,
    pub title: String,
    pub state: TicketState,
    pub priority: Priority,
    pub ticket_type: String,
    pub description: String,
}

/// A dependency edge between two tickets.
#[derive(Clone, Debug)]
pub struct TicketEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

// ---------------------------------------------------------------------------
// Bevy components
// ---------------------------------------------------------------------------

/// Component marking an entity as a ticket panel.
#[derive(Component, Clone, Debug)]
pub struct TicketPanel {
    pub ticket_id: String,
    pub state: TicketState,
    pub priority: Priority,
}

/// Component tracking a ticket panel's layout velocity (for force-directed layout).
#[derive(Component, Clone, Debug)]
pub struct TicketLayoutVelocity {
    pub velocity: Vec3,
}

impl Default for TicketLayoutVelocity {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
        }
    }
}

/// Tooltip display for hovered ticket panels.
#[derive(Component, Clone, Debug)]
pub struct TicketTooltip {
    pub summary: String,
}

// ---------------------------------------------------------------------------
// Bevy resources
// ---------------------------------------------------------------------------

/// Client-side cache of all tickets fetched from the ticket-api.
#[derive(Resource)]
pub struct TicketStore {
    pub tickets: HashMap<String, TicketData>,
    pub edges: Vec<TicketEdge>,
    /// Whether ticket data has changed and panels need rebuild.
    pub dirty: bool,
}

impl Default for TicketStore {
    fn default() -> Self {
        Self {
            tickets: HashMap::new(),
            edges: Vec::new(),
            dirty: false,
        }
    }
}

impl TicketStore {
    pub fn upsert(&mut self, ticket: TicketData) {
        self.tickets.insert(ticket.id.clone(), ticket);
        self.dirty = true;
    }

    pub fn remove(&mut self, id: &str) -> Option<TicketData> {
        self.dirty = true;
        self.tickets.remove(id)
    }

    pub fn get(&self, id: &str) -> Option<&TicketData> {
        self.tickets.get(id)
    }

    pub fn add_edge(&mut self, edge: TicketEdge) {
        self.edges.push(edge);
        self.dirty = true;
    }

    pub fn ticket_count(&self) -> usize {
        self.tickets.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get all tickets connected to a given ticket via edges.
    pub fn neighbors(&self, ticket_id: &str) -> Vec<&str> {
        let mut result = Vec::new();
        for edge in &self.edges {
            if edge.from == ticket_id {
                result.push(edge.to.as_str());
            } else if edge.to == ticket_id {
                result.push(edge.from.as_str());
            }
        }
        result
    }

    /// Tickets filtered by state.
    pub fn by_state(&self, state: TicketState) -> Vec<&TicketData> {
        self.tickets.values().filter(|t| t.state == state).collect()
    }

    /// Count of tickets in each state.
    pub fn state_counts(&self) -> HashMap<TicketState, usize> {
        let mut counts = HashMap::new();
        for ticket in self.tickets.values() {
            *counts.entry(ticket.state).or_insert(0) += 1;
        }
        counts
    }
}

/// Currently selected ticket (clicked in 3D).
#[derive(Resource, Default)]
pub struct TicketSelection {
    pub selected_id: Option<String>,
}

/// Layout mode for positioning ticket panels.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Grid,
    ForceDirected,
}

impl Default for LayoutMode {
    fn default() -> Self {
        Self::Grid
    }
}

// ---------------------------------------------------------------------------
// Layout functions
// ---------------------------------------------------------------------------

/// Compute grid positions for tickets (deterministic, no overlap).
pub fn grid_positions(ticket_ids: &[&str]) -> HashMap<String, Vec3> {
    let mut positions = HashMap::new();
    for (i, id) in ticket_ids.iter().enumerate() {
        let col = i % GRID_COLUMNS;
        let row = i / GRID_COLUMNS;
        let x = col as f32 * GRID_SPACING;
        let y = 0.0;
        let z = row as f32 * GRID_SPACING;
        positions.insert(id.to_string(), Vec3::new(x, y, z));
    }
    positions
}

/// Compute one step of force-directed layout.
///
/// Each ticket node repels all others (inverse-square) and connected edges
/// attract (spring force). Velocity is damped each step.
pub fn force_directed_step(
    positions: &mut HashMap<String, Vec3>,
    velocities: &mut HashMap<String, Vec3>,
    edges: &[TicketEdge],
    dt: f32,
) {
    let ids: Vec<String> = positions.keys().cloned().collect();

    // Repulsion between all pairs
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let pa = positions[&ids[i]];
            let pb = positions[&ids[j]];
            let delta = pa - pb;
            let dist = delta.length().max(MIN_REPULSION_DIST);
            let force_mag = REPULSION_K / (dist * dist);
            let force = delta.normalize_or_zero() * force_mag;

            if let Some(va) = velocities.get_mut(&ids[i]) {
                *va += force * dt;
            }
            if let Some(vb) = velocities.get_mut(&ids[j]) {
                *vb -= force * dt;
            }
        }
    }

    // Spring attraction along edges
    for edge in edges {
        if let (Some(&pa), Some(&pb)) = (positions.get(&edge.from), positions.get(&edge.to)) {
            let delta = pb - pa;
            let dist = delta.length();
            let force = delta.normalize_or_zero() * SPRING_K * dist;

            if let Some(va) = velocities.get_mut(&edge.from) {
                *va += force * dt;
            }
            if let Some(vb) = velocities.get_mut(&edge.to) {
                *vb -= force * dt;
            }
        }
    }

    // Apply velocity with damping and clamping
    for id in &ids {
        if let (Some(vel), Some(pos)) = (velocities.get_mut(id), positions.get_mut(id)) {
            *vel *= LAYOUT_DAMPING;
            let speed = vel.length();
            if speed > MAX_VELOCITY {
                *vel = vel.normalize() * MAX_VELOCITY;
            }
            *pos += *vel * dt;
        }
    }
}

// ---------------------------------------------------------------------------
// Voxel edge drawing
// ---------------------------------------------------------------------------

/// Draw a 3D voxel line between two world positions (Bresenham-like).
///
/// Returns the list of integer voxel coordinates along the line.
pub fn voxel_line_3d(from: Vec3, to: Vec3) -> Vec<(i32, i32, i32)> {
    let mut result = Vec::new();
    let delta = to - from;
    let steps = delta
        .abs()
        .max_element()
        .ceil()
        .max(1.0) as usize;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let p = from + delta * t;
        let ix = p.x.round() as i32;
        let iy = p.y.round() as i32;
        let iz = p.z.round() as i32;
        // Avoid duplicates
        if result.last() != Some(&(ix, iy, iz)) {
            result.push((ix, iy, iz));
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// System: run force-directed layout on ticket panels.
fn layout_system(
    time: Res<Time>,
    layout_mode: Res<LayoutMode>,
    store: Res<TicketStore>,
    mut query: Query<(&TicketPanel, &mut Transform, &mut TicketLayoutVelocity)>,
) {
    if *layout_mode != LayoutMode::ForceDirected {
        return;
    }

    let dt = time.delta_secs();
    if dt == 0.0 {
        return;
    }

    // Collect current positions and velocities
    let mut positions: HashMap<String, Vec3> = HashMap::new();
    let mut velocities: HashMap<String, Vec3> = HashMap::new();

    for (panel, transform, vel) in query.iter() {
        positions.insert(panel.ticket_id.clone(), transform.translation);
        velocities.insert(panel.ticket_id.clone(), vel.velocity);
    }

    // Step the layout
    force_directed_step(&mut positions, &mut velocities, &store.edges, dt);

    // Write back
    for (panel, mut transform, mut vel) in query.iter_mut() {
        if let Some(new_pos) = positions.get(&panel.ticket_id) {
            transform.translation = *new_pos;
        }
        if let Some(new_vel) = velocities.get(&panel.ticket_id) {
            vel.velocity = *new_vel;
        }
    }
}

/// System: handle ticket panel selection via click.
fn selection_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<TicketSelection>,
    query: Query<&TicketPanel>,
) {
    // A placeholder: in the full implementation, selection comes from
    // 3D ray-cast → panel hit test (T8d). Here we use Escape to deselect.
    if keyboard.just_pressed(KeyCode::Escape) {
        selection.selected_id = None;
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin that registers ticket-editor resources and systems.
pub struct TicketEditorPlugin;

impl Plugin for TicketEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TicketStore>();
        app.init_resource::<TicketSelection>();
        app.init_resource::<LayoutMode>();

        app.add_systems(
            Update,
            (
                layout_system,
                selection_system,
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- TicketState ---

    #[test]
    fn ticket_state_roundtrip() {
        for state in [
            TicketState::New,
            TicketState::InRefinement,
            TicketState::Ready,
            TicketState::InImplementation,
            TicketState::InReview,
            TicketState::InValidation,
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
        assert!(!TicketState::New.is_terminal());
        assert!(!TicketState::InImplementation.is_terminal());
    }

    #[test]
    fn ticket_state_tint_distinct() {
        let new_tint = TicketState::New.tint_color();
        let done_tint = TicketState::Done.tint_color();
        assert_ne!(new_tint, done_tint);
    }

    #[test]
    fn ticket_state_unknown_defaults_new() {
        assert_eq!(TicketState::from_str("bogus"), TicketState::New);
    }

    // --- Priority ---

    #[test]
    fn priority_roughness_order() {
        assert!(Priority::Critical.glass_roughness() < Priority::High.glass_roughness());
        assert!(Priority::High.glass_roughness() < Priority::Medium.glass_roughness());
        assert!(Priority::Medium.glass_roughness() < Priority::Low.glass_roughness());
    }

    #[test]
    fn priority_from_str() {
        assert_eq!(Priority::from_str("critical"), Priority::Critical);
        assert_eq!(Priority::from_str("high"), Priority::High);
        assert_eq!(Priority::from_str("unknown"), Priority::None);
    }

    // --- EdgeKind ---

    #[test]
    fn edge_kind_colors_distinct() {
        assert_ne!(EdgeKind::DependsOn.edge_color(), EdgeKind::Blocks.edge_color());
        assert_ne!(EdgeKind::Blocks.edge_color(), EdgeKind::RelatedTo.edge_color());
    }

    #[test]
    fn edge_kind_metallic() {
        assert!(EdgeKind::Blocks.is_metallic());
        assert!(!EdgeKind::DependsOn.is_metallic());
        assert!(!EdgeKind::RelatedTo.is_metallic());
    }

    // --- TicketStore ---

    #[test]
    fn store_upsert_and_get() {
        let mut store = TicketStore::default();
        store.upsert(TicketData {
            id: "abc".into(),
            title: "Test ticket".into(),
            state: TicketState::New,
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
            state: TicketState::New,
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
            state: TicketState::New,
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
            state: TicketState::New,
            priority: Priority::None,
            ticket_type: "task".into(),
            description: "".into(),
        });
        store.upsert(TicketData {
            id: "b".into(),
            title: "B".into(),
            state: TicketState::New,
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
            state: TicketState::New,
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
            state: TicketState::New,
            priority: Priority::None,
            ticket_type: "task".into(),
            description: "".into(),
        });
        assert_eq!(store.by_state(TicketState::New).len(), 2);
        assert_eq!(store.by_state(TicketState::Done).len(), 1);
        assert_eq!(store.by_state(TicketState::Cancelled).len(), 0);
    }

    #[test]
    fn store_state_counts() {
        let mut store = TicketStore::default();
        store.upsert(TicketData {
            id: "a".into(), title: "A".into(),
            state: TicketState::New, priority: Priority::None,
            ticket_type: "t".into(), description: "".into(),
        });
        store.upsert(TicketData {
            id: "b".into(), title: "B".into(),
            state: TicketState::New, priority: Priority::None,
            ticket_type: "t".into(), description: "".into(),
        });
        store.upsert(TicketData {
            id: "c".into(), title: "C".into(),
            state: TicketState::Done, priority: Priority::None,
            ticket_type: "t".into(), description: "".into(),
        });
        let counts = store.state_counts();
        assert_eq!(*counts.get(&TicketState::New).unwrap(), 2);
        assert_eq!(*counts.get(&TicketState::Done).unwrap(), 1);
    }

    // --- Grid layout ---

    #[test]
    fn grid_positions_no_overlap() {
        let ids = vec!["a", "b", "c", "d", "e", "f"];
        let positions = grid_positions(&ids);
        assert_eq!(positions.len(), 6);
        // Check no two positions are the same
        let pos_list: Vec<Vec3> = positions.values().cloned().collect();
        for i in 0..pos_list.len() {
            for j in (i + 1)..pos_list.len() {
                assert_ne!(pos_list[i], pos_list[j]);
            }
        }
    }

    #[test]
    fn grid_positions_respects_columns() {
        let ids: Vec<&str> = (0..10).map(|i| match i {
            0 => "a", 1 => "b", 2 => "c", 3 => "d", 4 => "e",
            5 => "f", 6 => "g", 7 => "h", 8 => "i", _ => "j",
        }).collect();
        let positions = grid_positions(&ids);
        // First row: z=0, second row: z=GRID_SPACING
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

    // --- Force-directed layout ---

    #[test]
    fn force_directed_step_repels() {
        let mut positions = HashMap::new();
        positions.insert("a".to_string(), Vec3::new(0.0, 0.0, 0.0));
        positions.insert("b".to_string(), Vec3::new(1.0, 0.0, 0.0));

        let mut velocities = HashMap::new();
        velocities.insert("a".to_string(), Vec3::ZERO);
        velocities.insert("b".to_string(), Vec3::ZERO);

        force_directed_step(&mut positions, &mut velocities, &[], 1.0);

        // After repulsion, a should move left (negative x) and b right (positive x)
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

        // At large distance, spring attraction should dominate repulsion
        // so a should move toward b (positive x)
        assert!(velocities["a"].x > 0.0, "a should be attracted toward b");
    }

    #[test]
    fn force_directed_velocity_clamped() {
        let mut positions = HashMap::new();
        positions.insert("a".to_string(), Vec3::ZERO);
        positions.insert("b".to_string(), Vec3::new(0.1, 0.0, 0.0)); // very close → huge repulsion

        let mut velocities = HashMap::new();
        velocities.insert("a".to_string(), Vec3::ZERO);
        velocities.insert("b".to_string(), Vec3::ZERO);

        force_directed_step(&mut positions, &mut velocities, &[], 1.0);

        assert!(velocities["a"].length() <= MAX_VELOCITY + 0.001);
        assert!(velocities["b"].length() <= MAX_VELOCITY + 0.001);
    }

    // --- Voxel line ---

    #[test]
    fn voxel_line_single_point() {
        let line = voxel_line_3d(Vec3::ZERO, Vec3::ZERO);
        assert_eq!(line.len(), 1);
        assert_eq!(line[0], (0, 0, 0));
    }

    #[test]
    fn voxel_line_horizontal() {
        let line = voxel_line_3d(Vec3::new(0.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
        assert!(line.len() >= 5);
        assert_eq!(line[0], (0, 0, 0));
        assert_eq!(*line.last().unwrap(), (5, 0, 0));
        // All y and z should be 0
        for &(_, y, z) in &line {
            assert_eq!(y, 0);
            assert_eq!(z, 0);
        }
    }

    #[test]
    fn voxel_line_diagonal() {
        let line = voxel_line_3d(Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 3.0, 3.0));
        assert!(!line.is_empty());
        assert_eq!(line[0], (0, 0, 0));
        assert_eq!(*line.last().unwrap(), (3, 3, 3));
    }

    #[test]
    fn voxel_line_no_duplicates() {
        let line = voxel_line_3d(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 5.0, 3.0));
        for i in 1..line.len() {
            assert_ne!(line[i], line[i - 1], "Duplicate entry at index {}", i);
        }
    }

    // --- TicketSelection ---

    #[test]
    fn selection_default_none() {
        let sel = TicketSelection::default();
        assert!(sel.selected_id.is_none());
    }
}
