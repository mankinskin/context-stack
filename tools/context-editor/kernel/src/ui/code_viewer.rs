//! Code Viewer: Source Code as Glass Panels Refracting Voxel Splats.
//!
//! Source code files are displayed as dark-tinted glass panels in the 3D
//! Voxel-splatted world. Code panels use moderate roughness — enough frosting
//! to keep syntax-highlighted text readable, but transparent enough to show the
//! voxel-splatted scene behind for spatial context.

use bevy::prelude::*;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default panel half-width for code panels.
pub const CODE_PANEL_HALF_WIDTH: f32 = 2.5;

/// Default panel half-height.
pub const CODE_PANEL_HALF_HEIGHT: f32 = 3.0;

/// Corner radius for code panels.
pub const CODE_CORNER_RADIUS: f32 = 0.05;

/// Default roughness (moderate frost — voxel scene softly visible behind code).
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
// Syntax token types (for highlighting)
// ---------------------------------------------------------------------------

/// Token category for syntax highlighting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Type,
    String,
    Number,
    Comment,
    Operator,
    Punctuation,
    Function,
    Plain,
}

impl TokenKind {
    /// RGBA color for this token kind (theme-derived).
    pub fn color(&self) -> u32 {
        match self {
            Self::Keyword => 0xC586C0FF,  // Purple
            Self::Type => 0x4EC9B0FF,     // Teal
            Self::String => 0xCE9178FF,   // Orange-brown
            Self::Number => 0xB5CEA8FF,   // Light green
            Self::Comment => 0x6A9955FF,  // Green
            Self::Operator => 0xD4D4D4FF, // Light gray
            Self::Punctuation => 0xD4D4D4FF,
            Self::Function => 0xDCDCAAFF, // Yellow
            Self::Plain => 0xD4D4D4FF,    // Light gray
        }
    }
}

/// A single syntax-highlighted token.
#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxToken {
    pub kind: TokenKind,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Simple keyword-based tokenizer
// ---------------------------------------------------------------------------

/// Rust keywords for simple highlighting.
const RUST_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "const", "pub", "struct", "enum", "impl", "use", "mod",
    "crate", "self", "super", "if", "else", "match", "for", "while", "loop",
    "return", "break", "continue", "where", "trait", "type", "as", "in", "ref",
    "move", "async", "await", "unsafe", "extern", "dyn",
];

/// Rust built-in types for highlighting.
const RUST_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
    "i128", "isize", "f32", "f64", "bool", "char", "str", "String", "Vec",
    "HashMap", "Option", "Result", "Box", "Self",
];

/// Tokenize a single line of Rust source code.
///
/// This is a simple keyword-based tokenizer, not a full parser.
pub fn tokenize_rust_line(line: &str) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();
    let mut buf = String::new();

    while let Some(&ch) = chars.peek() {
        // Comment
        if ch == '/' {
            chars.next();
            if chars.peek() == Some(&'/') {
                chars.next();
                let rest: String = chars.collect();
                flush_buf(&mut buf, &mut tokens);
                tokens.push(SyntaxToken {
                    kind: TokenKind::Comment,
                    text: format!("//{}", rest),
                });
                return tokens;
            } else {
                flush_buf(&mut buf, &mut tokens);
                tokens.push(SyntaxToken {
                    kind: TokenKind::Operator,
                    text: "/".into(),
                });
                continue;
            }
        }

        // String literal
        if ch == '"' {
            flush_buf(&mut buf, &mut tokens);
            let mut s = String::new();
            s.push(ch);
            chars.next();
            let mut escaped = false;
            while let Some(&c) = chars.peek() {
                s.push(c);
                chars.next();
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    break;
                }
            }
            tokens.push(SyntaxToken {
                kind: TokenKind::String,
                text: s,
            });
            continue;
        }

        // Numbers
        if ch.is_ascii_digit() && buf.is_empty() {
            flush_buf(&mut buf, &mut tokens);
            let mut num = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
                    num.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(SyntaxToken {
                kind: TokenKind::Number,
                text: num,
            });
            continue;
        }

        // Punctuation / operators
        if !ch.is_alphanumeric() && ch != '_' {
            flush_buf(&mut buf, &mut tokens);
            let kind = match ch {
                '+' | '-' | '*' | '=' | '!' | '<' | '>' | '&' | '|' | '^'
                | '%' => TokenKind::Operator,
                _ => TokenKind::Punctuation,
            };
            tokens.push(SyntaxToken {
                kind,
                text: ch.to_string(),
            });
            chars.next();
            continue;
        }

        // Word characters
        buf.push(ch);
        chars.next();

        // Check if next char is not a word char → flush
        let next_is_word = chars
            .peek()
            .map(|c| c.is_alphanumeric() || *c == '_')
            .unwrap_or(false);
        if !next_is_word {
            flush_buf(&mut buf, &mut tokens);
        }
    }

    flush_buf(&mut buf, &mut tokens);
    tokens
}

/// Flush accumulated buffer as a classified token.
fn flush_buf(
    buf: &mut String,
    tokens: &mut Vec<SyntaxToken>,
) {
    if buf.is_empty() {
        return;
    }
    let kind = if RUST_KEYWORDS.contains(&buf.as_str()) {
        TokenKind::Keyword
    } else if RUST_TYPES.contains(&buf.as_str()) {
        TokenKind::Type
    } else {
        TokenKind::Plain
    };
    tokens.push(SyntaxToken {
        kind,
        text: std::mem::take(buf),
    });
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
            content.lines().map(|l| l.to_string()).collect();
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
        let s = start.min(self.lines.len());
        let e = end.min(self.lines.len());
        &self.lines[s..e]
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
            .map(|f| f.line_count())
            .unwrap_or(0);
        panel.scroll(delta, total_lines);
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin registering code viewer resources and systems.
pub struct CodeViewerPlugin;

impl Plugin for CodeViewerPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<SourceFileCache>();
        app.init_resource::<CodeSelection>();

        app.add_systems(Update, scroll_code_system);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Language ---

    #[test]
    fn language_from_extension() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("ts"), Language::TypeScript);
        assert_eq!(Language::from_extension("tsx"), Language::TypeScript);
        assert_eq!(Language::from_extension("js"), Language::JavaScript);
        assert_eq!(Language::from_extension("wgsl"), Language::Wgsl);
        assert_eq!(Language::from_extension("toml"), Language::Toml);
        assert_eq!(Language::from_extension("md"), Language::Markdown);
        assert_eq!(Language::from_extension("xyz"), Language::Unknown);
    }

    #[test]
    fn language_from_path() {
        assert_eq!(Language::from_path("src/main.rs"), Language::Rust);
        assert_eq!(Language::from_path("package.json"), Language::Unknown);
        assert_eq!(Language::from_path("shader.wgsl"), Language::Wgsl);
    }

    #[test]
    fn language_label() {
        assert_eq!(Language::Rust.label(), "Rust");
        assert_eq!(Language::Unknown.label(), "Plain text");
    }

    // --- TokenKind ---

    #[test]
    fn token_colors_distinct() {
        assert_ne!(TokenKind::Keyword.color(), TokenKind::String.color());
        assert_ne!(TokenKind::Comment.color(), TokenKind::Number.color());
    }

    // --- Tokenizer ---

    #[test]
    fn tokenize_empty_line() {
        let tokens = tokenize_rust_line("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_keyword() {
        let tokens = tokenize_rust_line("fn main");
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword && t.text == "fn"));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Plain && t.text == "main"));
    }

    #[test]
    fn tokenize_type() {
        let tokens = tokenize_rust_line("let x: u32");
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword && t.text == "let"));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Type && t.text == "u32"));
    }

    #[test]
    fn tokenize_string_literal() {
        let tokens = tokenize_rust_line("let s = \"hello world\";");
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::String && t.text.contains("hello")));
    }

    #[test]
    fn tokenize_comment() {
        let tokens = tokenize_rust_line("// this is a comment");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Comment);
        assert!(tokens[0].text.contains("this is a comment"));
    }

    #[test]
    fn tokenize_number() {
        let tokens = tokenize_rust_line("42");
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Number && t.text == "42"));
    }

    #[test]
    fn tokenize_escaped_string() {
        let tokens = tokenize_rust_line(r#""hello \"world\"""#);
        // Should be one string token
        let string_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::String)
            .collect();
        assert_eq!(string_tokens.len(), 1);
    }

    #[test]
    fn tokenize_operators() {
        let tokens = tokenize_rust_line("a + b");
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Operator && t.text == "+"));
    }

    // --- SourceFile ---

    #[test]
    fn source_file_new() {
        let file = SourceFile::new(
            "src/main.rs".into(),
            "fn main() {\n    println!(\"hi\");\n}",
        );
        assert_eq!(file.language, Language::Rust);
        assert_eq!(file.line_count(), 3);
    }

    #[test]
    fn source_file_get_range() {
        let file =
            SourceFile::new("t.rs".into(), "line0\nline1\nline2\nline3\nline4");
        let range = file.get_range(1, 4);
        assert_eq!(range.len(), 3);
        assert_eq!(range[0], "line1");
        assert_eq!(range[2], "line3");
    }

    #[test]
    fn source_file_get_range_clamped() {
        let file = SourceFile::new("t.rs".into(), "a\nb");
        let range = file.get_range(0, 100);
        assert_eq!(range.len(), 2);
    }

    // --- CodePanel ---

    #[test]
    fn code_panel_default_visible_lines() {
        let panel = CodePanel::new("test.rs".into(), Language::Rust);
        assert_eq!(panel.visible_line_count(), DEFAULT_VISIBLE_LINES);
    }

    #[test]
    fn code_panel_scroll_down() {
        let mut panel = CodePanel::new("test.rs".into(), Language::Rust);
        panel.scroll(10, 200);
        assert_eq!(panel.visible_start, 10);
        assert_eq!(panel.visible_end, 10 + DEFAULT_VISIBLE_LINES);
    }

    #[test]
    fn code_panel_scroll_up_clamped() {
        let mut panel = CodePanel::new("test.rs".into(), Language::Rust);
        panel.scroll(-5, 200);
        assert_eq!(panel.visible_start, 0); // can't go below 0
    }

    #[test]
    fn code_panel_scroll_past_end() {
        let mut panel = CodePanel::new("test.rs".into(), Language::Rust);
        let total = 60;
        panel.scroll(100, total);
        // window = 50, max_start = 60 - 50 = 10
        assert_eq!(panel.visible_start, 10);
        assert_eq!(panel.visible_end, 60);
    }

    #[test]
    fn code_panel_highlight() {
        let mut panel = CodePanel::new("test.rs".into(), Language::Rust);
        panel.highlight_lines = vec![5, 10, 15];
        assert!(panel.is_line_highlighted(5));
        assert!(panel.is_line_highlighted(10));
        assert!(!panel.is_line_highlighted(7));
    }

    // --- SourceFileCache ---

    #[test]
    fn cache_insert_and_get() {
        let mut cache = SourceFileCache::default();
        cache.insert(SourceFile::new("a.rs".into(), "fn a() {}"));
        assert_eq!(cache.count(), 1);
        assert!(cache.get("a.rs").is_some());
        assert!(cache.get("b.rs").is_none());
    }

    // --- CodeSelection ---

    #[test]
    fn selection_default() {
        let sel = CodeSelection::default();
        assert!(sel.file_path.is_none());
        assert!(sel.line.is_none());
    }
}
