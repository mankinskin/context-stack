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

        if trimmed.starts_with("```") {
            in_code_block = true;
            code_lang = trimmed[3..].trim().to_string();
            continue;
        }

        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count() as u8;
            let text = trimmed[level as usize..].trim().to_string();
            elements.push(MarkdownElement::Heading { level, text });
            continue;
        }

        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            elements.push(MarkdownElement::HorizontalRule);
            continue;
        }

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            elements.push(MarkdownElement::ListItem(trimmed[2..].to_string()));
            continue;
        }

        if let Some(xref) = parse_cross_reference(trimmed) {
            elements.push(xref);
            continue;
        }

        elements.push(MarkdownElement::Paragraph(trimmed.to_string()));
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
