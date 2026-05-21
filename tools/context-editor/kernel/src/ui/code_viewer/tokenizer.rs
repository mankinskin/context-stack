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
            Self::Keyword => 0xC586C0FF,
            Self::Type => 0x4EC9B0FF,
            Self::String => 0xCE9178FF,
            Self::Number => 0xB5CEA8FF,
            Self::Comment => 0x6A9955FF,
            Self::Operator => 0xD4D4D4FF,
            Self::Punctuation => 0xD4D4D4FF,
            Self::Function => 0xDCDCAAFF,
            Self::Plain => 0xD4D4D4FF,
        }
    }
}

/// A single syntax-highlighted token.
#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxToken {
    pub kind: TokenKind,
    pub text: String,
}

const RUST_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "const", "pub", "struct", "enum", "impl", "use", "mod",
    "crate", "self", "super", "if", "else", "match", "for", "while", "loop",
    "return", "break", "continue", "where", "trait", "type", "as", "in", "ref",
    "move", "async", "await", "unsafe", "extern", "dyn",
];

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

        buf.push(ch);
        chars.next();

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
