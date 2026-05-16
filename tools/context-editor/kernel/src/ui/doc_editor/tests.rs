use super::*;

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
        MarkdownElement::Heading {
            level: 1,
            text: "Title".into()
        }
    );
    assert_eq!(
        elements[1],
        MarkdownElement::Heading {
            level: 2,
            text: "Subtitle".into()
        }
    );
}

#[test]
fn parse_paragraph() {
    let elements = parse_markdown("Hello world");
    assert_eq!(
        elements,
        vec![MarkdownElement::Paragraph("Hello world".into())]
    );
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
    assert!(matches!(
        elements[0],
        MarkdownElement::Heading { level: 1, .. }
    ));
}

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
    assert_eq!(doc.approx_line_count(), 5);
}

#[test]
fn store_insert_and_get() {
    let mut store = DocStore::default();
    store.insert(Document::new(
        "d1".into(),
        "Doc 1".into(),
        DocType::Guide,
        "body".into(),
    ));
    assert_eq!(store.count(), 1);
    assert_eq!(store.get("d1").unwrap().title, "Doc 1");
}

#[test]
fn store_remove() {
    let mut store = DocStore::default();
    store.insert(Document::new(
        "d1".into(),
        "Doc 1".into(),
        DocType::Guide,
        "".into(),
    ));
    assert!(store.remove("d1").is_some());
    assert_eq!(store.count(), 0);
}

#[test]
fn store_by_type() {
    let mut store = DocStore::default();
    store.insert(Document::new(
        "d1".into(),
        "A".into(),
        DocType::Guide,
        "".into(),
    ));
    store.insert(Document::new(
        "d2".into(),
        "B".into(),
        DocType::CrateDoc,
        "".into(),
    ));
    store.insert(Document::new(
        "d3".into(),
        "C".into(),
        DocType::Guide,
        "".into(),
    ));
    assert_eq!(store.by_type(DocType::Guide).len(), 2);
    assert_eq!(store.by_type(DocType::CrateDoc).len(), 1);
    assert_eq!(store.by_type(DocType::Readme).len(), 0);
}

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
