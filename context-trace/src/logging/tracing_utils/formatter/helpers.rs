//! Helper functions and types for trait context extraction

/// Trait context extracted from span fields
#[derive(Debug)]
pub(super) struct TraitContext {
    pub(super) trait_name: Option<String>,
    pub(super) self_type: Option<String>,
    pub(super) associated_types: Vec<(String, String)>,
}

/// Extract trait context from formatted fields string
/// Looks for special fields: self_type, trait_name, and patterns like next_type, error_type, etc.
pub(super) fn extract_trait_context(fields_str: &str) -> Option<TraitContext> {
    let mut trait_name = None;
    let mut self_type = None;
    let mut associated_types = Vec::new();

    // Simple parsing - look for patterns like self_type="..." or trait_name="..."
    // This assumes fields are formatted as: field="value" or field=value

    // Extract self_type
    if let Some(idx) = fields_str.find("self_type=") {
        let start = idx + 10; // Skip 'self_type='
        let remaining = &fields_str[start..];
        if let Some(value) = extract_field_value(remaining) {
            self_type = Some(value);
        }
    }

    // Extract trait_name
    if let Some(idx) = fields_str.find("trait_name=") {
        let start = idx + 11; // Skip 'trait_name='
        let remaining = &fields_str[start..];
        if let Some(value) = extract_field_value(remaining) {
            trait_name = Some(value);
        }
    }

    // Extract associated types (fields ending with _type but not self_type)
    for part in fields_str.split_whitespace() {
        if part.contains("_type=")
            && !part.starts_with("self_type=")
            && !part.starts_with("trait_name=")
            && let Some(eq_pos) = part.find('=')
        {
            let field_name = &part[..eq_pos];
            let remaining = &part[eq_pos + 1..];
            if let Some(value) = extract_field_value(remaining) {
                // Convert next_type to "Next", error_type to "Error", etc.
                let assoc_name = field_name
                    .strip_suffix("_type")
                    .map(|s| {
                        // Capitalize first letter
                        let mut chars = s.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) =>
                                first.to_uppercase().chain(chars).collect(),
                        }
                    })
                    .unwrap_or_else(|| field_name.to_string());
                associated_types.push((assoc_name, value));
            }
        }
    }

    if trait_name.is_some()
        || self_type.is_some()
        || !associated_types.is_empty()
    {
        Some(TraitContext {
            trait_name,
            self_type,
            associated_types,
        })
    } else {
        None
    }
}

/// Extract a field value from a string, handling quoted and unquoted values
fn extract_field_value(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(stripped) = s.strip_prefix('"') {
        stripped.find('"').map(|end| s[1..end + 1].to_string())
    } else {
        // Unquoted value - take until whitespace
        let end = s.find(char::is_whitespace).unwrap_or(s.len());
        if end > 0 {
            Some(s[..end].to_string())
        } else {
            None
        }
    }
}
