//! Rust Debug format to JSON conversion
//!
//! Transforms Rust Debug-formatted values (e.g., `StructName { field: value }`)
//! into structured JSON objects for log file post-processing.
//!
//! This module handles the conversion of string fields in JSON log entries
//! that contain Rust Debug output, function signatures, or serialized data
//! into proper structured JSON objects.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        Mutex,
    },
};

/// Thread-safe store for collecting function signatures during logging.
///
/// Maps function name to its parsed fn_sig JSON object.
/// Signatures are collected during log writing and later dumped to
/// `target/debug_signatures/<test_name>.json`.
pub type SignatureStore = Arc<Mutex<HashMap<String, serde_json::Value>>>;

/// Create a new empty signature store.
pub fn new_signature_store() -> SignatureStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Convert Windows paths to Unix paths in a string
fn to_unix_path(s: &str) -> String {
    // Replace backslashes with forward slashes
    let mut result = s.replace('\\', "/");
    // Collapse consecutive slashes (e.g., // -> /) but preserve :// for URLs
    while result.contains("//") {
        result = result.replace("//", "/");
    }
    result
}

/// Parse a Rust function signature into a JSON object
/// e.g., "fn foo(&mut self, x: Type) -> Result" -> {"name": "foo", "params": [...], "return_type": "Result"}
fn parse_fn_signature(s: &str) -> Option<serde_json::Value> {
    let s = s.trim();
    if !s.starts_with("fn ") {
        return None;
    }

    // Extract function name
    let after_fn = &s[3..];
    let name_end = after_fn.find('(')?;
    let name = after_fn[..name_end].trim();

    // Find params between first ( and matching )
    let params_start = s.find('(')?;
    let mut depth = 0;
    let mut params_end = None;
    for (i, c) in s[params_start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    params_end = Some(params_start + i);
                    break;
                }
            },
            _ => {},
        }
    }
    let params_end = params_end?;
    let params_str = &s[params_start + 1..params_end];

    // Parse params - split by comma but respect nested brackets
    let params = parse_param_list(params_str);

    // Extract return type if present
    let after_params = &s[params_end + 1..];
    let return_type = if let Some(arrow_pos) = after_params.find("->") {
        Some(after_params[arrow_pos + 2..].trim().to_string())
    } else {
        None
    };

    let mut obj = serde_json::Map::new();
    obj.insert(
        "name".to_string(),
        serde_json::Value::String(name.to_string()),
    );
    obj.insert("params".to_string(), serde_json::Value::Array(params));
    if let Some(ret) = return_type {
        obj.insert("return_type".to_string(), serde_json::Value::String(ret));
    }

    Some(serde_json::Value::Object(obj))
}

/// Parse a comma-separated parameter list respecting nested brackets
fn parse_param_list(s: &str) -> Vec<serde_json::Value> {
    let mut params = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in s.chars() {
        match c {
            '(' | '[' | '<' | '{' => {
                depth += 1;
                current.push(c);
            },
            ')' | ']' | '>' | '}' => {
                depth -= 1;
                current.push(c);
            },
            ',' if depth == 0 => {
                let param = current.trim();
                if !param.is_empty() {
                    params.push(parse_param(param));
                }
                current.clear();
            },
            _ => current.push(c),
        }
    }

    let param = current.trim();
    if !param.is_empty() {
        params.push(parse_param(param));
    }

    params
}

/// Parse a single parameter like "&mut self" or "x: Type" into JSON
fn parse_param(s: &str) -> serde_json::Value {
    let s = s.trim();

    // Handle self parameters
    if s == "self" || s == "&self" || s == "&mut self" {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "self".to_string(),
            serde_json::Value::String(s.to_string()),
        );
        return serde_json::Value::Object(obj);
    }

    // Try to split name: type
    if let Some(colon_pos) = s.find(':') {
        let name = s[..colon_pos].trim();
        let ty = s[colon_pos + 1..].trim();
        let mut obj = serde_json::Map::new();
        obj.insert(
            "name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
        obj.insert(
            "type".to_string(),
            serde_json::Value::String(ty.to_string()),
        );
        return serde_json::Value::Object(obj);
    }

    // Fallback: just return as string
    serde_json::Value::String(s.to_string())
}

/// Parse a token-like value: "text"(index) -> {"text": "text", "index": index}
fn parse_token_value(s: &str) -> Option<serde_json::Value> {
    let s = s.trim();

    // Pattern: "text"(number) or 'text'(number)
    if !s.starts_with('"') && !s.starts_with('\'') {
        return None;
    }

    let quote_char = s.chars().next()?;
    let end_quote = s[1..].find(quote_char)?;
    let text = &s[1..end_quote + 1];

    let after_quote = &s[end_quote + 2..];
    if !after_quote.starts_with('(') || !after_quote.ends_with(')') {
        return None;
    }

    let index_str = &after_quote[1..after_quote.len() - 1];
    let index: i64 = index_str.parse().ok()?;

    let mut obj = serde_json::Map::new();
    obj.insert(
        "text".to_string(),
        serde_json::Value::String(text.to_string()),
    );
    obj.insert("index".to_string(), serde_json::Value::Number(index.into()));

    Some(serde_json::Value::Object(obj))
}

/// Map of known enum variants to their parent enum type
fn get_enum_parent(variant: &str) -> Option<&'static str> {
    match variant {
        "Some" | "None" => Some("Option"),
        "Ok" | "Err" => Some("Result"),
        _ => None,
    }
}

/// Try to parse a Rust Debug-formatted struct, e.g., "StructName { field: value }"
fn parse_rust_debug(s: &str) -> Option<serde_json::Value> {
    let s = s.trim();

    // Don't try to parse arrays as structs
    if s.starts_with('[') {
        return None;
    }

    // Handle unit variants like "None" without braces
    if s == "None" {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "_type".to_string(),
            serde_json::Value::String("Option".to_string()),
        );
        obj.insert(
            "_variant".to_string(),
            serde_json::Value::String("None".to_string()),
        );
        return Some(serde_json::Value::Object(obj));
    }

    // Check if it looks like "Name { ... }" or "Name(...)"
    let brace_pos = s.find(|c| c == '{' || c == '(');
    let brace_pos = brace_pos?;

    let struct_name = s[..brace_pos].trim();

    // struct_name must be a valid identifier (alphanumeric, ::, _, no quotes/brackets)
    if struct_name.is_empty()
        || struct_name.contains(' ')
        || struct_name.contains('"')
        || struct_name.contains('[')
        || struct_name.contains(']')
    {
        return None;
    }

    let open_brace = s.chars().nth(brace_pos)?;
    let close_brace = if open_brace == '{' { '}' } else { ')' };

    // Find matching close brace
    let content_start = brace_pos + 1;
    let mut depth = 1;
    let mut content_end = None;
    for (i, c) in s[content_start..].char_indices() {
        if c == open_brace {
            depth += 1;
        } else if c == close_brace {
            depth -= 1;
            if depth == 0 {
                content_end = Some(content_start + i);
                break;
            }
        }
    }
    let content_end = content_end?;
    let content = &s[content_start..content_end];

    let mut obj = serde_json::Map::new();

    // Check if this is a known enum variant (e.g., Some, Ok, Err)
    // If so, use the parent enum as _type and add _variant
    if let Some(parent_enum) = get_enum_parent(struct_name) {
        obj.insert(
            "_type".to_string(),
            serde_json::Value::String(parent_enum.to_string()),
        );
        obj.insert(
            "_variant".to_string(),
            serde_json::Value::String(struct_name.to_string()),
        );
    } else {
        // Regular struct/enum - use the name directly as _type
        obj.insert(
            "_type".to_string(),
            serde_json::Value::String(struct_name.to_string()),
        );
    }

    // Parse fields
    if open_brace == '{' {
        // Named fields: "field: value, field2: value2"
        parse_struct_fields(content, &mut obj);
    } else {
        // Tuple fields: "value1, value2"
        let values = parse_tuple_fields(content);
        if !values.is_empty() {
            obj.insert("_values".to_string(), serde_json::Value::Array(values));
        }
    }

    Some(serde_json::Value::Object(obj))
}

/// Parse struct fields like "field: value, field2: value2"
fn parse_struct_fields(
    s: &str,
    obj: &mut serde_json::Map<String, serde_json::Value>,
) {
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_value = false;
    let mut depth = 0;

    for c in s.chars() {
        match c {
            '(' | '[' | '<' | '{' => {
                depth += 1;
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            },
            ')' | ']' | '>' | '}' => {
                depth -= 1;
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            },
            ':' if depth == 0 && !in_value => {
                in_value = true;
            },
            ',' if depth == 0 => {
                let key = current_key.trim();
                let value = current_value.trim();
                if !key.is_empty() {
                    obj.insert(key.to_string(), try_parse_value(value));
                }
                current_key.clear();
                current_value.clear();
                in_value = false;
            },
            _ =>
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                },
        }
    }

    // Handle last field
    let key = current_key.trim();
    let value = current_value.trim();
    if !key.is_empty() {
        obj.insert(key.to_string(), try_parse_value(value));
    }
}

/// Parse tuple fields
fn parse_tuple_fields(s: &str) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in s.chars() {
        match c {
            '(' | '[' | '<' | '{' => {
                depth += 1;
                current.push(c);
            },
            ')' | ']' | '>' | '}' => {
                depth -= 1;
                current.push(c);
            },
            ',' if depth == 0 => {
                let value = current.trim();
                if !value.is_empty() {
                    values.push(try_parse_value(value));
                }
                current.clear();
            },
            _ => current.push(c),
        }
    }

    let value = current.trim();
    if !value.is_empty() {
        values.push(try_parse_value(value));
    }

    values
}

/// Try to parse a value into a more structured form
fn try_parse_value(s: &str) -> serde_json::Value {
    let s = s.trim();

    // Try parsing as a Rust array [...] first
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1];
        let values = parse_tuple_fields(inner);
        return serde_json::Value::Array(values);
    }

    // Try parsing as a Rust HashMap/BTreeMap {key: value, ...}
    // Only if not valid JSON (JSON is handled separately)
    if s.starts_with('{') && s.ends_with('}') {
        if serde_json::from_str::<serde_json::Value>(s).is_err() {
            let inner = &s[1..s.len() - 1];
            let mut obj = serde_json::Map::new();
            parse_struct_fields(inner, &mut obj);
            if !obj.is_empty() {
                return serde_json::Value::Object(obj);
            }
        }
    }

    // Try parsing as a token like "a"(0) -> {"text": "a", "index": 0}
    if let Some(parsed) = parse_token_value(s) {
        return parsed;
    }

    // Try parsing as a nested struct
    if let Some(parsed) = parse_rust_debug(s) {
        return parsed;
    }

    // Handle PhantomData<...> by extracting the type parameter
    if s.starts_with("PhantomData<") && s.ends_with('>') {
        let inner = &s[12..s.len() - 1];
        let mut obj = serde_json::Map::new();
        obj.insert(
            "_type".to_string(),
            serde_json::Value::String("PhantomData".to_string()),
        );
        obj.insert(
            "type_param".to_string(),
            serde_json::Value::String(inner.to_string()),
        );
        return serde_json::Value::Object(obj);
    }

    // Handle quoted strings
    if s.starts_with('"') && s.ends_with('"') {
        return serde_json::Value::String(s[1..s.len() - 1].to_string());
    }

    // Try parsing as number
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::Value::Number(n.into());
    }
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return serde_json::Value::Number(num);
        }
    }

    // Boolean
    if s == "true" {
        return serde_json::Value::Bool(true);
    }
    if s == "false" {
        return serde_json::Value::Bool(false);
    }

    // Unit types like ()
    if s == "()" {
        return serde_json::Value::Null;
    }

    // Fallback: return as string
    serde_json::Value::String(s.to_string())
}

/// Check if a string looks like it might be a parseable Rust debug value
fn looks_like_rust_debug(s: &str) -> bool {
    let s = s.trim();
    // Struct/Enum patterns: Name { ... }, Name(...), Name
    // Array pattern: [...]
    // HashMap pattern: {key: value, ...}
    // Token pattern: "text"(num)
    // Option/Result: Some(...), None, Ok(...), Err(...)
    // With path: path::Name { ... }
    s.starts_with('[')
        || (s.starts_with('{') && s.ends_with('}'))
        || s.starts_with('"') && s.contains('(')
        || s.starts_with("Some(")
        || s.starts_with("None")
        || s.starts_with("Ok(")
        || s.starts_with("Err(")
        || s.starts_with("PhantomData")
        || (s
            .chars()
            .next()
            .map(|c| c.is_ascii_uppercase())
            .unwrap_or(false)
            && (s.contains('{') || s.contains('(') || s.contains("::")))
}

/// Check if a string looks like a JSON object or array
fn looks_like_json(s: &str) -> bool {
    let s = s.trim();
    (s.starts_with('{') && s.ends_with('}'))
        || (s.starts_with('[') && s.ends_with(']'))
}

/// Try to parse a string value into a structured JSON value.
///
/// Attempts parsing in this order:
/// 1. `fn_sig` fields: parse as function signature
/// 2. JSON strings (starting with `{` or `[`): parse as JSON objects/arrays
/// 3. Rust Debug output (e.g., `StructName { field: value }`): parse as structured JSON
///
/// Returns `Some(parsed)` if a non-string value was produced, `None` otherwise.
fn try_parse_string_to_json(
    key: &str,
    s: &str,
) -> Option<serde_json::Value> {
    // Special handling for fn_sig - parse function signatures
    if key == "fn_sig" {
        if let Some(parsed) = parse_fn_signature(s) {
            return Some(parsed);
        }
    }

    // Try parsing as a JSON string (object or array)
    if looks_like_json(s) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
            // Only return if it actually parsed into an object or array (not a plain string)
            if !matches!(parsed, serde_json::Value::String(_)) {
                return Some(parsed);
            }
        }
    }

    // Try parsing as Rust Debug formatted struct/enum
    if looks_like_rust_debug(s) {
        let parsed = try_parse_value(s);
        if !matches!(parsed, serde_json::Value::String(_)) {
            return Some(parsed);
        }
    }

    None
}

/// Transform ALL string fields that look like JSON or Rust debug values into structured JSON objects.
///
/// Also collects fn_sig entries into the signature store (if provided) and
/// strips them from the output to reduce log file size.
pub fn transform_structured_fields(
    value: &mut serde_json::Value,
    signatures: Option<&SignatureStore>,
) {
    match value {
        serde_json::Value::Object(obj) => {
            // Process each key-value pair
            let keys: Vec<String> = obj.keys().cloned().collect();
            for key in keys {
                if let Some(val) = obj.get_mut(&key) {
                    // First, recursively transform nested objects/arrays
                    transform_structured_fields(val, signatures);

                    // Then try to parse string values into structured JSON
                    if let serde_json::Value::String(s) = val {
                        if let Some(parsed) = try_parse_string_to_json(&key, s)
                        {
                            *val = parsed;
                        }
                    }
                }
            }

            // Collect fn_sig from span/spans objects and strip them
            collect_and_strip_fn_sigs(obj, signatures);
        },
        serde_json::Value::Array(arr) =>
            for item in arr {
                transform_structured_fields(item, signatures);
            },
        _ => {},
    }
}

/// Collect fn_sig and self_type from span objects, store in signatures,
/// then simplify spans to plain name strings.
///
/// Looks for fn_sig/self_type in:
/// - `span` (current span object)
/// - `spans[*]` (span stack objects)
/// - `fields.fn_sig` (event fields inherited from span)
///
/// After collection, span objects `{"name": "foo", "self_type": "..."}` become
/// plain strings `"foo"`, and self_type is merged into the fn_sig entry.
fn collect_and_strip_fn_sigs(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    signatures: Option<&SignatureStore>,
) {
    /// Extract fn_sig and self_type from a span object, collect into store.
    /// Returns the span name if found.
    fn collect_from_span_obj(
        span_obj: &mut serde_json::Map<String, serde_json::Value>,
        signatures: Option<&SignatureStore>,
    ) -> Option<String> {
        let name = span_obj
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        let fn_sig = span_obj.remove("fn_sig");
        let self_type = span_obj.remove("self_type");

        if let Some(store) = signatures {
            if let Some(ref fn_name) = name {
                if let Ok(mut sigs) = store.lock() {
                    // Insert fn_sig if we have one, merging self_type into it
                    if let Some(mut sig) = fn_sig {
                        if let Some(self_type_val) = &self_type {
                            if let Some(sig_obj) = sig.as_object_mut() {
                                sig_obj
                                    .entry("self_type".to_string())
                                    .or_insert(self_type_val.clone());
                            }
                        }
                        sigs.entry(fn_name.clone()).or_insert(sig);
                    } else if let Some(self_type_val) = &self_type {
                        // No fn_sig but we have self_type — add it to existing entry
                        if let Some(existing) = sigs.get_mut(fn_name) {
                            if let Some(obj) = existing.as_object_mut() {
                                obj.entry("self_type".to_string())
                                    .or_insert(self_type_val.clone());
                            }
                        }
                    }
                }
            }
        }

        name
    }

    // Process "span" object → replace with name string
    if let Some(serde_json::Value::Object(span_obj)) = obj.get_mut("span") {
        if let Some(name) = collect_from_span_obj(span_obj, signatures) {
            obj.insert("span".to_string(), serde_json::Value::String(name));
        }
    }

    // Process "spans" array → collect signatures, then replace each object with name string
    if let Some(serde_json::Value::Array(spans)) = obj.get_mut("spans") {
        // First pass: collect fn_sig and self_type from each span object
        for span in spans.iter_mut() {
            if let serde_json::Value::Object(span_obj) = span {
                let _ = collect_from_span_obj(span_obj, signatures);
            }
        }
        // Second pass: replace objects with name strings
        let simplified: Vec<serde_json::Value> = spans
            .iter()
            .map(|s| match s {
                serde_json::Value::Object(obj) =>
                    obj.get("name").cloned().unwrap_or(serde_json::Value::Null),
                other => other.clone(),
            })
            .collect();
        *spans = simplified;
    }

    // Process "fields" object (fn_sig can appear here too)
    if let Some(serde_json::Value::Object(fields_obj)) = obj.get_mut("fields") {
        if let Some(fn_sig) = fields_obj.remove("fn_sig") {
            if let Some(store) = signatures {
                if let Some(name) = fn_sig.get("name").and_then(|n| n.as_str())
                {
                    if let Ok(mut sigs) = store.lock() {
                        sigs.entry(name.to_string()).or_insert(fn_sig);
                    }
                }
            }
        }
        // Also strip self_type from fields
        fields_obj.remove("self_type");
    }
}

/// Recursively convert all path-like strings in a JSON value to Unix format
pub fn convert_paths_to_unix(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            // Convert if it looks like a path (contains backslash)
            if s.contains('\\') {
                *s = to_unix_path(s);
            }
        },
        serde_json::Value::Array(arr) =>
            for item in arr {
                convert_paths_to_unix(item);
            },
        serde_json::Value::Object(obj) =>
            for (_, v) in obj {
                convert_paths_to_unix(v);
            },
        _ => {},
    }
}
