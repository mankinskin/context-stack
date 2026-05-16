//! Documentation Editor: Frosted Glass Panels in Voxel-Splatted World.
//!
//! Documentation pages (from doc-viewer / MCP doc sources) are displayed as
//! frosted glass panels in the 3D scene. Docs use higher roughness for
//! readability — the mipmap-blurred background provides ambient context
//! without distracting from text.

use bevy::prelude::*;
use std::collections::HashMap;

mod markdown;
mod plugin;

pub use markdown::{
    parse_markdown,
    MarkdownElement,
};
pub use plugin::DocEditorPlugin;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default panel half-width (world units) — docs are wider for text.
pub const DOC_PANEL_HALF_WIDTH: f32 = 2.0;

/// Default panel half-height — tall for text content.
pub const DOC_PANEL_HALF_HEIGHT: f32 = 3.0;

/// Corner radius for doc panels.
pub const DOC_CORNER_RADIUS: f32 = 0.08;

/// Default roughness for doc panels (high frost for readability).
pub const DOC_ROUGHNESS: f32 = 0.6;

/// Default cool tint for doc panels (slight blue).
pub const DOC_TINT: (f32, f32, f32) = (0.95, 0.95, 1.0);

/// Pixel height of a single text line in the rendered content texture.
pub const LINE_HEIGHT_PX: u32 = 20;

/// Maximum visible lines before virtual scrolling kicks in.
pub const MAX_VISIBLE_LINES: usize = 60;

/// Scroll speed (lines per scroll event).
pub const SCROLL_SPEED: f32 = 3.0;

/// Camera lerp speed when navigating to a cross-reference target.
pub const CAMERA_LERP_SPEED: f32 = 3.0;

// ---------------------------------------------------------------------------
// Document types
// ---------------------------------------------------------------------------

/// Classification of a documentation source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocType {
    AgentDoc,
    CrateDoc,
    Guide,
    Readme,
}

impl DocType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "agent_doc" | "agent" => Self::AgentDoc,
            "crate_doc" | "crate" => Self::CrateDoc,
            "guide" => Self::Guide,
            "readme" => Self::Readme,
            _ => Self::Guide,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AgentDoc => "Agent Doc",
            Self::CrateDoc => "Crate Doc",
            Self::Guide => "Guide",
            Self::Readme => "README",
        }
    }

    /// Roughness modifier per doc type (additive to base).
    pub fn roughness_modifier(&self) -> f32 {
        match self {
            Self::CrateDoc => -0.05, // slightly less frosted (API focus)
            Self::Guide => 0.05,     // more frosted (reading focus)
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A document loaded from the doc-viewer API.
#[derive(Clone, Debug)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub doc_type: DocType,
    pub source: String,
    /// Pre-parsed elements (cached).
    pub elements: Vec<MarkdownElement>,
}

impl Document {
    pub fn new(
        id: String,
        title: String,
        doc_type: DocType,
        source: String,
    ) -> Self {
        let elements = parse_markdown(&source);
        Self {
            id,
            title,
            doc_type,
            source,
            elements,
        }
    }

    /// Count headings in the document (for table-of-contents).
    pub fn heading_count(&self) -> usize {
        self.elements
            .iter()
            .filter(|e| matches!(e, MarkdownElement::Heading { .. }))
            .count()
    }

    /// Extract all cross-reference target IDs.
    pub fn cross_references(&self) -> Vec<&str> {
        self.elements
            .iter()
            .filter_map(|e| match e {
                MarkdownElement::CrossReference { target_id, .. } =>
                    Some(target_id.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Total line count (approximate: paragraphs = 1 line, code blocks = N lines).
    pub fn approx_line_count(&self) -> usize {
        self.elements
            .iter()
            .map(|e| match e {
                MarkdownElement::CodeBlock { code, .. } =>
                    code.lines().count().max(1),
                _ => 1,
            })
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Bevy components
// ---------------------------------------------------------------------------

/// Component marking an entity as a documentation panel.
#[derive(Component, Clone, Debug)]
pub struct DocPanel {
    pub doc_id: String,
    pub doc_type: DocType,
    pub scroll_offset: f32,
}

/// Component for doc panels reachable via cross-reference navigation.
#[derive(Component, Clone, Debug)]
pub struct DocCrossRefTarget {
    pub doc_id: String,
}

// ---------------------------------------------------------------------------
// Bevy resources
// ---------------------------------------------------------------------------

/// Client-side document cache.
#[derive(Resource)]
pub struct DocStore {
    pub documents: HashMap<String, Document>,
    pub dirty: bool,
}

impl Default for DocStore {
    fn default() -> Self {
        Self {
            documents: HashMap::new(),
            dirty: false,
        }
    }
}

impl DocStore {
    pub fn insert(
        &mut self,
        doc: Document,
    ) {
        self.documents.insert(doc.id.clone(), doc);
        self.dirty = true;
    }

    pub fn get(
        &self,
        id: &str,
    ) -> Option<&Document> {
        self.documents.get(id)
    }

    pub fn remove(
        &mut self,
        id: &str,
    ) -> Option<Document> {
        self.dirty = true;
        self.documents.remove(id)
    }

    pub fn count(&self) -> usize {
        self.documents.len()
    }

    pub fn by_type(
        &self,
        doc_type: DocType,
    ) -> Vec<&Document> {
        self.documents
            .values()
            .filter(|d| d.doc_type == doc_type)
            .collect()
    }
}

/// Navigation target for smooth camera lerp to a cross-referenced panel.
#[derive(Resource, Default)]
pub struct DocNavigation {
    pub target_position: Option<Vec3>,
    pub lerp_progress: f32,
}

impl DocNavigation {
    pub fn navigate_to(
        &mut self,
        pos: Vec3,
    ) {
        self.target_position = Some(pos);
        self.lerp_progress = 0.0;
    }

    pub fn is_navigating(&self) -> bool {
        self.target_position.is_some() && self.lerp_progress < 1.0
    }

    pub fn complete(&mut self) {
        self.target_position = None;
        self.lerp_progress = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// System: scroll doc panels based on input.
fn scroll_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panels: Query<&mut DocPanel>,
) {
    let scroll_delta = if keyboard.pressed(KeyCode::PageDown) {
        SCROLL_SPEED
    } else if keyboard.pressed(KeyCode::PageUp) {
        -SCROLL_SPEED
    } else {
        return;
    };

    for mut panel in panels.iter_mut() {
        panel.scroll_offset = (panel.scroll_offset + scroll_delta).max(0.0);
    }
}

/// System: interpolate camera toward a cross-reference navigation target.
fn navigation_system(
    time: Res<Time>,
    mut nav: ResMut<DocNavigation>,
    mut camera_q: Query<&mut Transform, With<Camera3d>>,
) {
    if !nav.is_navigating() {
        return;
    }

    let target = match nav.target_position {
        Some(t) => t,
        None => return,
    };

    let dt = time.delta_secs();
    nav.lerp_progress = (nav.lerp_progress + dt * CAMERA_LERP_SPEED).min(1.0);

    if let Ok(mut cam_transform) = camera_q.single_mut() {
        let current = cam_transform.translation;
        cam_transform.translation = current.lerp(target, nav.lerp_progress);

        if nav.lerp_progress >= 1.0 {
            nav.complete();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
