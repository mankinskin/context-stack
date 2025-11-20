//! Syntax highlighting utilities

use syntect::{
    easy::HighlightLines,
    highlighting::{
        Style,
        ThemeSet,
    },
    parsing::SyntaxSet,
    util::as_24_bit_terminal_escaped,
};

/// Syntax highlight a Rust function signature
pub(super) fn highlight_rust_signature(
    signature: &str,
    with_ansi: bool,
) -> String {
    if !with_ansi {
        return signature.to_string();
    }

    lazy_static::lazy_static! {
        static ref SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_newlines();
        static ref THEME_SET: ThemeSet = ThemeSet::load_defaults();
    }

    let syntax = SYNTAX_SET
        .find_syntax_by_extension("rs")
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
    let theme = &THEME_SET.themes["base16-ocean.dark"];

    let mut highlighted = String::new();
    let mut highlighter = HighlightLines::new(syntax, theme);
    for line in syntect::util::LinesWithEndings::from(signature) {
        let ranges: Vec<(Style, &str)> = highlighter
            .highlight_line(line, &SYNTAX_SET)
            .unwrap_or_default();

        highlighted.push_str(&as_24_bit_terminal_escaped(&ranges, false));
    }

    highlighted
}
