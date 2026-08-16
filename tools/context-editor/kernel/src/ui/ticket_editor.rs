//! Ticket Editor: Tickets as Glass Panels in Voxel-Splatted World.
//!
//! Each ticket from the ticket-api is rendered as an interactive 3D glass SDF
//! panel displaying ticket fields. Dependency edges are drawn as voxel lines
//! that participate in splat generation, creating soft glowing connections.

use bevy::prelude::*;
use std::collections::HashMap;

mod layout;
mod plugin;

pub use layout::{
    force_directed_step,
    grid_positions,
    voxel_line_3d,
};
pub use plugin::TicketEditorPlugin;

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
    Open,
    Planned,
    InImplementation,
    InReview,
    Done,
    Cancelled,
}

impl TicketState {
    pub fn from_str(s: &str) -> Self {
        match s {
            "open" => Self::Open,
            "planned" => Self::Planned,
            "in-implementation" => Self::InImplementation,
            "in-review" => Self::InReview,
            "done" => Self::Done,
            "cancelled" => Self::Cancelled,
            _ => Self::Open,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Planned => "planned",
            Self::InImplementation => "in-implementation",
            Self::InReview => "in-review",
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
            Self::Open => (0.3, 0.5, 1.0),              // Blue
            Self::Planned => (0.3, 0.8, 0.5),           // Teal-green
            Self::InImplementation => (1.0, 0.85, 0.2), // Yellow
            Self::InReview => (1.0, 0.6, 0.2),          // Orange
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
            Self::DependsOn => 0xFF8844FF, // Orange — dependency
            Self::Blocks => 0xFF4444FF,    // Red — blocker
            Self::RelatedTo => 0x88AAFFFF, // Light blue — informational
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
    pub fn upsert(
        &mut self,
        ticket: TicketData,
    ) {
        self.tickets.insert(ticket.id.clone(), ticket);
        self.dirty = true;
    }

    pub fn remove(
        &mut self,
        id: &str,
    ) -> Option<TicketData> {
        self.dirty = true;
        self.tickets.remove(id)
    }

    pub fn get(
        &self,
        id: &str,
    ) -> Option<&TicketData> {
        self.tickets.get(id)
    }

    pub fn add_edge(
        &mut self,
        edge: TicketEdge,
    ) {
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
    pub fn neighbors(
        &self,
        ticket_id: &str,
    ) -> Vec<&str> {
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
    pub fn by_state(
        &self,
        state: TicketState,
    ) -> Vec<&TicketData> {
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
    _query: Query<&TicketPanel>,
) {
    // A placeholder: in the full implementation, selection comes from
    // 3D ray-cast → panel hit test (T8d). Here we use Escape to deselect.
    if keyboard.just_pressed(KeyCode::Escape) {
        selection.selected_id = None;
    }
}

#[cfg(test)]
mod tests;
