use super::*;

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

#[test]
fn token_colors_distinct() {
    assert_ne!(TokenKind::Keyword.color(), TokenKind::String.color());
    assert_ne!(TokenKind::Comment.color(), TokenKind::Number.color());
}

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
    let tokens = tokenize_rust_line(r#"\"hello \\\"world\\\"\""#);
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
    assert_eq!(panel.visible_start, 0);
}

#[test]
fn code_panel_scroll_past_end() {
    let mut panel = CodePanel::new("test.rs".into(), Language::Rust);
    let total = 60;
    panel.scroll(100, total);
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

#[test]
fn cache_insert_and_get() {
    let mut cache = SourceFileCache::default();
    cache.insert(SourceFile::new("a.rs".into(), "fn a() {}"));
    assert_eq!(cache.count(), 1);
    assert!(cache.get("a.rs").is_some());
    assert!(cache.get("b.rs").is_none());
}

#[test]
fn selection_default() {
    let sel = CodeSelection::default();
    assert!(sel.file_path.is_none());
    assert!(sel.line.is_none());
}
