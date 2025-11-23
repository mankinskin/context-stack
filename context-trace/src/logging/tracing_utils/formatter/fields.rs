//! Field filtering and display logic

use super::field_visitor::FieldVisitor;

/// Filter span fields by removing special fields that are shown elsewhere
pub(super) fn filter_span_fields(fields: &str) -> String {
    let mut cleaned = fields.to_string();

    // Remove fn_sig if it was shown inline with span name
    let has_fn_sig = fields.contains("fn_sig=");
    if has_fn_sig {
        cleaned = remove_field(&cleaned, "fn_sig");
    }

    // Remove message field as it's shown separately
    cleaned = remove_field(&cleaned, "message");

    // Remove trait context fields
    cleaned = remove_field(&cleaned, "self_type");
    cleaned = remove_field(&cleaned, "trait_name");

    // Remove associated type fields (ending with _type)
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    cleaned = words
        .into_iter()
        .filter(|word| {
            if let Some(eq_pos) = word.find('=') {
                let field_name = &word[..eq_pos];
                !field_name.ends_with("_type") || field_name == "self_type"
            } else {
                true
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    cleaned.trim().to_string()
}

/// Remove a field from formatted fields string
fn remove_field(
    fields: &str,
    field_name: &str,
) -> String {
    let pattern = format!("{}=", field_name);

    if let Some(start) = fields.find(&pattern) {
        // Find where the value ends (at next space or end of string)
        let after_eq = start + pattern.len();
        let remaining = &fields[after_eq..];

        let value_end = if remaining.starts_with('"') {
            // Quoted value - find closing quote
            remaining[1..]
                .find('"')
                .map(|pos| after_eq + pos + 2)
                .unwrap_or(fields.len())
        } else {
            // Unquoted value - find next space
            remaining
                .find(char::is_whitespace)
                .map(|pos| after_eq + pos)
                .unwrap_or(fields.len())
        };

        // Remove the field and normalize whitespace
        let before = fields[..start].trim_end();
        let after = fields[value_end..].trim_start();

        if before.is_empty() {
            after.to_string()
        } else if after.is_empty() {
            before.to_string()
        } else {
            format!("{} {}", before, after)
        }
    } else {
        fields.to_string()
    }
}

/// Format non-message fields for regular events
pub(super) fn format_event_fields(formatted_fields: &str) -> String {
    // Parse fields - this is simplified, in reality we'd need proper field iteration
    // For now, just return the cleaned fields
    let cleaned = remove_field(formatted_fields, "message");
    cleaned.trim().to_string()
}
