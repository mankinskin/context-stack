//! Documentation Editor: Frosted Glass Panels in Voxel-Splatted World.
//!
//! Documentation pages (from doc-viewer / MCP doc sources) are displayed as
//! frosted glass panels in the 3D scene. Docs use higher roughness for
//! readability — the mipmap-blurred background provides ambient context
//! without distracting from text.

use bevy::prelude::*;
use std::collections::HashMap;

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
// Markdown parsing (lightweight)
// ---------------------------------------------------------------------------

/// Kind of element in a parsed markdown document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownElement {
    Heading { level: u8, text: String },
    Paragraph(String),
    CodeBlock { language: String, code: String },
    ListItem(String),
    HorizontalRule,
    CrossReference { target_id: String, label: String },
}

/// Parse a simple markdown string into structured elements.
///
/// This is a lightweight parser — handles headings, code blocks, list items,
/// horizontal rules, and cross-references `[label](#target_id)`.
pub fn parse_markdown(source: &str) -> Vec<MarkdownElement> {
    let mut elements = Vec::new();
    let mut lines = source.lines().peekable();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    while let Some(line) = lines.next() {
        if in_code_block {
            if line.trim_start().starts_with("```") {
                elements.push(MarkdownElement::CodeBlock {
                    language: code_lang.clone(),
                    code: code_buf.clone(),
                });
                in_code_block = false;
                code_buf.clear();
                code_lang.clear();
            } else {
                if !code_buf.is_empty() {
                    code_buf.push('\n');
                }
                code_buf.push_str(line);
            }
            continue;
        }

        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Code block start
        if trimmed.starts_with("```") {
            in_code_block = true;
            code_lang = trimmed[3..].trim().to_string();
            continue;
        }

        // Heading
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count() as u8;
            let text = trimmed[level as usize..].trim().to_string();
            elements.push(MarkdownElement::Heading { level, text });
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            elements.push(MarkdownElement::HorizontalRule);
            continue;
        }

        // List item
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            elements.push(MarkdownElement::ListItem(trimmed[2..].to_string()));
            continue;
        }

        // Cross-reference: [label](#target_id)
        if let Some(xref) = parse_cross_reference(trimmed) {
            elements.push(xref);
            continue;
        }

        // Default: paragraph
        elements.push(MarkdownElement::Paragraph(trimmed.to_string()));
    }

    // Handle unclosed code block
    if in_code_block && !code_buf.is_empty() {
        elements.push(MarkdownElement::CodeBlock {
            language: code_lang,
            code: code_buf,
        });
    }

    elements
}

/// Try to parse a `[label](#target_id)` cross-reference from a line.
fn parse_cross_reference(line: &str) -> Option<MarkdownElement> {
    // Simple pattern: entire line is [label](#id)
    if !line.starts_with('[') {
        return None;
    }
    let close_bracket = line.find(']')?;
    let label = line[1..close_bracket].to_string();
    let rest = &line[close_bracket + 1..];
    if !rest.starts_with("(#") {
        return None;
    }
    let close_paren = rest.find(')')?;
    let target_id = rest[2..close_paren].to_string();
    Some(MarkdownElement::CrossReference { target_id, label })
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
    pub fn new(id: String, title: String, doc_type: DocType, source: String) -> Self {
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
                MarkdownElement::CrossReference { target_id, .. } => Some(target_id.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Total line count (approximate: paragraphs = 1 line, code blocks = N lines).
    pub fn approx_line_count(&self) -> usize {
        self.elements
            .iter()
            .map(|e| match e {
                MarkdownElement::CodeBlock { code, .. } => code.lines().count().max(1),
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
    pub fn insert(&mut self, doc: Document) {
        self.documents.insert(doc.id.clone(), doc);
        self.dirty = true;
    }

    pub fn get(&self, id: &str) -> Option<&Document> {
        self.documents.get(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Document> {
        self.dirty = true;
        self.documents.remove(id)
    }

    pub fn count(&self) -> usize {
        self.documents.len()
    }

    pub fn by_type(&self, doc_type: DocType) -> Vec<&Document> {
        self.documents.values().filter(|d| d.doc_type == doc_type).collect()
    }
}

/// Navigation target for smooth camera lerp to a cross-referenced panel.
#[derive(Resource, Default)]
pub struct DocNavigation {
    pub target_position: Option<Vec3>,
    pub lerp_progress: f32,
}

impl DocNavigation {
    pub fn navigate_to(&mut self, pos: Vec3) {
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
// Plugin
// ---------------------------------------------------------------------------

/// Plugin that registers documentation editor resources and systems.
pub struct DocEditorPlugin;

impl Plugin for DocEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DocStore>();
        app.init_resource::<DocNavigation>();

        app.add_systems(
            Update,
            (
                scroll_system,
                navigation_system,
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

    // --- DocType ---

    #[test]
    fn doc_type_from_str() {
        assert_eq!(DocType::from_str("agent_doc"), DocType::AgentDoc);
        assert_eq!(DocType::from_str("agent"), DocType::AgentDoc);
        assert_eq!(DocType::from_str("crate_doc"), DocType::CrateDoc);
        assert_eq!(DocType::from_str("guide"), DocType::Guide);
        assert_eq!(DocType::from_str("readme"), DocType::Readme);
        assert_eq!(DocType::from_str("unknown"), DocType::Guide);
    }

    #[test]
    fn doc_type_label() {
        assert_eq!(DocType::AgentDoc.label(), "Agent Doc");
        assert_eq!(DocType::CrateDoc.label(), "Crate Doc");
    }

    #[test]
    fn doc_type_roughness_modifier() {
        assert!(DocType::CrateDoc.roughness_modifier() < 0.0);
        assert!(DocType::Guide.roughness_modifier() > 0.0);
        assert_eq!(DocType::AgentDoc.roughness_modifier(), 0.0);
    }

    // --- Markdown parsing ---

    #[test]
    fn parse_empty() {
        let elements = parse_markdown("");
        assert!(elements.is_empty());
    }

    #[test]
    fn parse_heading() {
        let elements = parse_markdown("# Title\n## Subtitle");
        assert_eq!(elements.len(), 2);
        assert_eq!(
            elements[0],
            MarkdownElement::Heading { level: 1, text: "Title".into() }
        );
        assert_eq!(
            elements[1],
            MarkdownElement::Heading { level: 2, text: "Subtitle".into() }
        );
    }

    #[test]
    fn parse_paragraph() {
        let elements = parse_markdown("Hello world");
        assert_eq!(elements, vec![MarkdownElement::Paragraph("Hello world".into())]);
    }

    #[test]
    fn parse_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 1);
        if let MarkdownElement::CodeBlock { language, code } = &elements[0] {
            assert_eq!(language, "rust");
            assert_eq!(code, "fn main() {}");
        } else {
            panic!("Expected CodeBlock");
        }
    }

    #[test]
    fn parse_code_block_multiline() {
        let md = "```\nline1\nline2\nline3\n```";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 1);
        if let MarkdownElement::CodeBlock { code, .. } = &elements[0] {
            assert_eq!(code, "line1\nline2\nline3");
        } else {
            panic!("Expected CodeBlock");
        }
    }

    #[test]
    fn parse_list_items() {
        let md = "- Item one\n- Item two\n* Item three";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0], MarkdownElement::ListItem("Item one".into()));
        assert_eq!(elements[2], MarkdownElement::ListItem("Item three".into()));
    }

    #[test]
    fn parse_horizontal_rule() {
        let elements = parse_markdown("---");
        assert_eq!(elements, vec![MarkdownElement::HorizontalRule]);
    }

    #[test]
    fn parse_cross_reference() {
        let md = "[See also](#doc-abc)";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 1);
        assert_eq!(
            elements[0],
            MarkdownElement::CrossReference {
                target_id: "doc-abc".into(),
                label: "See also".into()
            }
        );
    }

    #[test]
    fn parse_mixed_document() {
        let md = "\
# API Guide

Some intro text.

```rust
fn hello() {}
```

- Point one
- Point two

---

[Next section](#sec2)
";
        let elements = parse_markdown(md);
        assert!(elements.len() >= 6);
        assert!(matches!(elements[0], MarkdownElement::Heading { level: 1, .. }));
    }

    // --- Document ---

    #[test]
    fn document_heading_count() {
        let doc = Document::new(
            "d1".into(),
            "Test".into(),
            DocType::Guide,
            "# H1\n## H2\nParagraph\n### H3".into(),
        );
        assert_eq!(doc.heading_count(), 3);
    }

    #[test]
    fn document_cross_references() {
        let doc = Document::new(
            "d1".into(),
            "Test".into(),
            DocType::Guide,
            "Text\n[link](#target1)\nMore\n[link2](#target2)".into(),
        );
        let refs = doc.cross_references();
        assert_eq!(refs, vec!["target1", "target2"]);
    }

    #[test]
    fn document_approx_line_count() {
        let doc = Document::new(
            "d1".into(),
            "Test".into(),
            DocType::Guide,
            "# Title\nParagraph\n```\ncode1\ncode2\ncode3\n```".into(),
        );
        // heading(1) + paragraph(1) + code(3 lines) = 5
        assert_eq!(doc.approx_line_count(), 5);
    }

    // --- DocStore ---

    #[test]
    fn store_insert_and_get() {
        let mut store = DocStore::default();
        store.insert(Document::new("d1".into(), "Doc 1".into(), DocType::Guide, "body".into()));
        assert_eq!(store.count(), 1);
        assert_eq!(store.get("d1").unwrap().title, "Doc 1");
    }

    #[test]
    fn store_remove() {
        let mut store = DocStore::default();
        store.insert(Document::new("d1".into(), "Doc 1".into(), DocType::Guide, "".into()));
        assert!(store.remove("d1").is_some());
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn store_by_type() {
        let mut store = DocStore::default();
        store.insert(Document::new("d1".into(), "A".into(), DocType::Guide, "".into()));
        store.insert(Document::new("d2".into(), "B".into(), DocType::CrateDoc, "".into()));
        store.insert(Document::new("d3".into(), "C".into(), DocType::Guide, "".into()));
        assert_eq!(store.by_type(DocType::Guide).len(), 2);
        assert_eq!(store.by_type(DocType::CrateDoc).len(), 1);
        assert_eq!(store.by_type(DocType::Readme).len(), 0);
    }

    // --- DocNavigation ---

    #[test]
    fn navigation_default_not_navigating() {
        let nav = DocNavigation::default();
        assert!(!nav.is_navigating());
    }

    #[test]
    fn navigation_navigate_and_complete() {
        let mut nav = DocNavigation::default();
        nav.navigate_to(Vec3::new(10.0, 5.0, 0.0));
        assert!(nav.is_navigating());

        nav.lerp_progress = 1.0;
        assert!(!nav.is_navigating());

        nav.complete();
        assert!(nav.target_position.is_none());
    }
}
