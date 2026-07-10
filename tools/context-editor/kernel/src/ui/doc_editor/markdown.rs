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

fn finish_code_block(
    elements: &mut Vec<MarkdownElement>,
    code_lang: &mut String,
    code_buf: &mut String,
    in_code_block: &mut bool,
) {
    elements.push(MarkdownElement::CodeBlock {
        language: code_lang.clone(),
        code: code_buf.clone(),
    });
    *in_code_block = false;
    code_buf.clear();
    code_lang.clear();
}

fn append_code_line(
    code_buf: &mut String,
    line: &str,
) {
    if !code_buf.is_empty() {
        code_buf.push('\n');
    }
    code_buf.push_str(line);
}

fn parse_markdown_line(trimmed: &str) -> Option<MarkdownElement> {
    if trimmed.starts_with('#') {
        let level = trimmed.chars().take_while(|&ch| ch == '#').count() as u8;
        let text = trimmed[level as usize..].trim().to_string();
        return Some(MarkdownElement::Heading { level, text });
    }

    if trimmed == "---" || trimmed == "***" || trimmed == "___" {
        return Some(MarkdownElement::HorizontalRule);
    }

    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return Some(MarkdownElement::ListItem(trimmed[2..].to_string()));
    }

    parse_cross_reference(trimmed)
        .or_else(|| Some(MarkdownElement::Paragraph(trimmed.to_string())))
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
                finish_code_block(
                    &mut elements,
                    &mut code_lang,
                    &mut code_buf,
                    &mut in_code_block,
                );
            } else {
                append_code_line(&mut code_buf, line);
            }
            continue;
        }

        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("```") {
            in_code_block = true;
            code_lang = trimmed[3..].trim().to_string();
            continue;
        }

        if let Some(element) = parse_markdown_line(trimmed) {
            elements.push(element);
        }
    }

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
