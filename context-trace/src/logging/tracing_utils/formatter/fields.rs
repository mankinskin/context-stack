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
    // We need to preserve newlines, so process line by line
    let mut result_lines = Vec::new();
    for line in cleaned.lines() {
        let trimmed = line.trim();
        // Skip lines that are just field names ending with _type
        if let Some(eq_pos) = trimmed.find('=') {
            let field_name = &trimmed[..eq_pos];
            if field_name.ends_with("_type") && field_name != "self_type" {
                continue; // Skip this line
            }
        }
        result_lines.push(line);
    }

    let final_cleaned = result_lines.join("\n");
    final_cleaned.trim().to_string()
}

/// Remove a field from formatted fields string
/// Handles multi-line field values by finding the next field start
fn remove_field(
    fields: &str,
    field_name: &str,
) -> String {
    let pattern = format!("{}=", field_name);

    if let Some(start) = fields.find(&pattern) {
        // Find where this field ends - either at the next field or end of string
        // A new field starts with "\n    <name>=" pattern
        let after_eq = start + pattern.len();
        let remaining = &fields[after_eq..];

        // Find the next field by looking for newline followed by non-whitespace and '='
        let value_end = remaining
            .char_indices()
            .skip(1) // Skip first char to allow for immediate newline
            .find(|(i, c)| {
                if *c == '\n' {
                    // Check if next line starts a new field (has '=' before line break)
                    let rest = &remaining[*i + 1..];
                    rest.trim_start().contains('=')
                        && rest[..rest.find('\n').unwrap_or(rest.len())]
                            .contains('=')
                } else {
                    false
                }
            })
            .map(|(i, _)| after_eq + i)
            .unwrap_or(fields.len());

        // Remove the field preserving structure
        let before = &fields[..start];
        let after = &fields[value_end..];

        format!("{}{}", before, after)
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
