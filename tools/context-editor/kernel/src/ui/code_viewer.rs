//! Code Viewer: Source Code as Glass Panels Refracting Voxel Splats.
//!
//! Source code files are displayed as dark-tinted glass panels in the 3D
//! Voxel-splatted world. Code panels use moderate roughness - enough frosting
//! to keep syntax-highlighted text readable, but transparent enough to show the
//! voxel-splatted scene behind for spatial context.

use bevy::prelude::*;
use std::collections::HashMap;

mod plugin;
mod tokenizer;

pub use plugin::CodeViewerPlugin;
pub use tokenizer::{
    tokenize_rust_line,
    SyntaxToken,
    TokenKind,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default panel half-width for code panels.
pub const CODE_PANEL_HALF_WIDTH: f32 = 2.5;

/// Default panel half-height.
pub const CODE_PANEL_HALF_HEIGHT: f32 = 3.0;

/// Corner radius for code panels.
pub const CODE_CORNER_RADIUS: f32 = 0.05;

/// Default roughness (moderate frost - voxel scene softly visible behind code).
pub const CODE_ROUGHNESS: f32 = 0.4;

/// Dark tint for code readability.
pub const CODE_TINT: (f32, f32, f32) = (0.1, 0.1, 0.12);

/// Default number of visible lines per panel.
pub const DEFAULT_VISIBLE_LINES: usize = 50;

/// Maximum lines to render in a single code panel.
pub const MAX_VISIBLE_LINES: usize = 100;

/// Scroll speed in lines per scroll event.
pub const SCROLL_SPEED: f32 = 3.0;

/// Highlight overlay alpha for search results / errors.
pub const HIGHLIGHT_ALPHA: f32 = 0.25;

// ---------------------------------------------------------------------------
// Language classification
// ---------------------------------------------------------------------------

/// Programming language for syntax highlighting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Wgsl,
    Toml,
    Markdown,
    Unknown,
}

impl Language {
    /// Detect language from file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => Self::Rust,
            "ts" | "tsx" => Self::TypeScript,
            "js" | "jsx" => Self::JavaScript,
            "wgsl" => Self::Wgsl,
            "toml" => Self::Toml,
            "md" => Self::Markdown,
            _ => Self::Unknown,
        }
    }

    /// Detect language from a full file path.
    pub fn from_path(path: &str) -> Self {
        path.rsplit('.')
            .next()
            .map(Self::from_extension)
            .unwrap_or(Self::Unknown)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::TypeScript => "TypeScript",
            Self::JavaScript => "JavaScript",
            Self::Wgsl => "WGSL",
            Self::Toml => "TOML",
            Self::Markdown => "Markdown",
            Self::Unknown => "Plain text",
        }
    }
}

// ---------------------------------------------------------------------------
// Source file
// ---------------------------------------------------------------------------

/// A loaded source file for display in a code panel.
#[derive(Clone, Debug)]
pub struct SourceFile {
    pub path: String,
    pub language: Language,
    pub lines: Vec<String>,
}

impl SourceFile {
    pub fn new(
        path: String,
        content: &str,
    ) -> Self {
        let language = Language::from_path(&path);
        let lines: Vec<String> =
            content.lines().map(|line| line.to_string()).collect();
        Self {
            path,
            language,
            lines,
        }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a range of lines (0-indexed, clamped).
    pub fn get_range(
        &self,
        start: usize,
        end: usize,
    ) -> &[String] {
        let start = start.min(self.lines.len());
        let end = end.min(self.lines.len());
        &self.lines[start..end]
    }
}

// ---------------------------------------------------------------------------
// Bevy components
// ---------------------------------------------------------------------------

/// Component marking an entity as a code panel.
#[derive(Component, Clone, Debug)]
pub struct CodePanel {
    pub file_path: String,
    pub language: Language,
    pub visible_start: usize,
    pub visible_end: usize,
    pub scroll_offset: f32,
    pub highlight_lines: Vec<usize>,
}

impl CodePanel {
    pub fn new(
        file_path: String,
        language: Language,
    ) -> Self {
        Self {
            file_path,
            language,
            visible_start: 0,
            visible_end: DEFAULT_VISIBLE_LINES,
            scroll_offset: 0.0,
            highlight_lines: Vec::new(),
        }
    }

    pub fn visible_line_count(&self) -> usize {
        self.visible_end.saturating_sub(self.visible_start)
    }

    pub fn is_line_highlighted(
        &self,
        line: usize,
    ) -> bool {
        self.highlight_lines.contains(&line)
    }

    /// Scroll by a given number of lines, clamping to valid range.
    pub fn scroll(
        &mut self,
        delta_lines: i32,
        total_lines: usize,
    ) {
        let new_start =
            (self.visible_start as i32 + delta_lines).max(0) as usize;
        let window = self.visible_line_count();
        let max_start = total_lines.saturating_sub(window);
        self.visible_start = new_start.min(max_start);
        self.visible_end = (self.visible_start + window).min(total_lines);
    }
}

/// Component linking a code panel to a graph node for navigation.
#[derive(Component, Clone, Debug)]
pub struct CodeNodeLink {
    pub node_id: u64,
}

// ---------------------------------------------------------------------------
// Bevy resources
// ---------------------------------------------------------------------------

/// Cache of loaded source files.
#[derive(Resource, Default)]
pub struct SourceFileCache {
    pub files: HashMap<String, SourceFile>,
}

impl SourceFileCache {
    pub fn insert(
        &mut self,
        file: SourceFile,
    ) {
        self.files.insert(file.path.clone(), file);
    }

    pub fn get(
        &self,
        path: &str,
    ) -> Option<&SourceFile> {
        self.files.get(path)
    }

    pub fn count(&self) -> usize {
        self.files.len()
    }
}

/// Currently selected line in the focused code panel.
#[derive(Resource, Default)]
pub struct CodeSelection {
    pub file_path: Option<String>,
    pub line: Option<usize>,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// System: scroll code panels based on keyboard input.
fn scroll_code_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    cache: Res<SourceFileCache>,
    mut panels: Query<&mut CodePanel>,
) {
    let delta = if keyboard.pressed(KeyCode::PageDown) {
        SCROLL_SPEED as i32
    } else if keyboard.pressed(KeyCode::PageUp) {
        -(SCROLL_SPEED as i32)
    } else {
        return;
    };

    for mut panel in panels.iter_mut() {
        let total_lines = cache
            .get(&panel.file_path)
            .map(|file| file.line_count())
            .unwrap_or(0);
        panel.scroll(delta, total_lines);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
