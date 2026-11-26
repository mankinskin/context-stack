//! Output budget: fit a `Response` JSON into `max_output` bytes by
//! shortening lists and render text item-wise, never cutting JSON text.

use serde_json::{
    Map,
    Value,
    json,
};

/// JSON pointers of the lists that may be shortened.
const LISTS: &[&str] = &[
    "/ok/graphs/graphs",
    "/ok/search/contexts/items",
    "/ok/search/occurrences/items",
    "/ok/search/parents",
    "/ok/inputs/items",
    "/ok/vertex/parents",
    "/ok/vertex/composition",
    "/ok/read/inputs",
    "/ok/read/skipped",
    "/ok/render/graph/vertices",
    "/ok/render/graph/edges",
    "/ok/log_list/logs",
    "/ok/log_entries/entries",
    "/ok/log_query_result/entries",
    "/ok/log_search_result/results",
];

/// JSON pointers of strings that may be shortened at line boundaries.
const TEXTS: &[&str] = &["/ok/render/text"];

const HINT: &str = "output exceeded the MCP output budget and lists were shortened; \
                    ask for fewer items (limit), a smaller width/label_width, or render a \
                    subgraph with root and depth";

/// The serialized result and whether it had to be replaced by an error.
pub(crate) struct Fitted {
    pub text: String,
    pub overflow: bool,
}

/// Shortened JSON objects have sorted keys (`serde_json::Value` without
/// `preserve_order`).
pub(crate) fn fit(
    mut value: Value,
    max_output: usize,
) -> Fitted {
    let text = to_string(&value);
    if text.len() <= max_output {
        return Fitted {
            text,
            overflow: false,
        };
    }
    let original_bytes = text.len();
    let mut shortened = Map::new();
    loop {
        let Some(pointer) = largest_shrinkable(&value) else {
            return Fitted {
                text: overflow_error(&value, original_bytes, max_output),
                overflow: true,
            };
        };
        shrink(&mut value, pointer, &mut shortened);
        mark(&mut value, &shortened);
        let text = to_string(&value);
        if text.len() <= max_output {
            return Fitted {
                text,
                overflow: false,
            };
        }
    }
}

fn to_string(value: &Value) -> String {
    serde_json::to_string(value).expect("JSON values serialize")
}

fn largest_shrinkable(value: &Value) -> Option<&'static str> {
    LISTS
        .iter()
        .chain(TEXTS)
        .filter_map(|pointer| {
            let size = match value.pointer(pointer)? {
                Value::Array(items) if !items.is_empty() =>
                    to_string(&Value::Array(items.clone())).len(),
                Value::String(text) if !text.is_empty() => text.len(),
                _ => return None,
            };
            Some((size, *pointer))
        })
        .max_by_key(|(size, _)| *size)
        .map(|(_, pointer)| pointer)
}

fn shrink(
    value: &mut Value,
    pointer: &'static str,
    shortened: &mut Map<String, Value>,
) {
    let entry = shortened.get(pointer).cloned();
    match value.pointer_mut(pointer) {
        Some(Value::Array(items)) => {
            let total = entry
                .as_ref()
                .and_then(|e| e["total"].as_u64())
                .unwrap_or(items.len() as u64);
            items.truncate(items.len() / 2);
            shortened.insert(
                pointer.into(),
                json!({ "kept": items.len(), "total": total }),
            );
            // Lists with a `truncated` flag next to `items` report it too.
            if let Some(parent) = pointer.strip_suffix("/items")
                && let Some(Value::Object(list)) = value.pointer_mut(parent)
                && list.contains_key("truncated")
            {
                list.insert("truncated".into(), Value::Bool(true));
            }
        },
        Some(Value::String(text)) => {
            let total = entry
                .as_ref()
                .and_then(|e| e["total_bytes"].as_u64())
                .unwrap_or(text.len() as u64);
            let mut cut = text.len() / 2;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            let cut =
                text[..cut].rfind('\n').map_or(cut, |line_end| line_end + 1);
            text.truncate(cut);
            shortened.insert(
                pointer.into(),
                json!({ "kept_bytes": text.len(), "total_bytes": total }),
            );
        },
        _ => {},
    }
}

fn mark(
    value: &mut Value,
    shortened: &Map<String, Value>,
) {
    if let Value::Object(object) = value {
        object.insert("truncated_output".into(), Value::Bool(true));
        object
            .insert("truncated_items".into(), Value::Object(shortened.clone()));
        object.insert("truncated_hint".into(), Value::String(HINT.into()));
    }
}

/// Nothing left to shorten: a `too_large` error response instead.
fn overflow_error(
    value: &Value,
    bytes: usize,
    max_output: usize,
) -> String {
    let mut error = json!({
        "error": {
            "kind": "too_large",
            "message": format!(
                "result of {bytes} bytes exceeds the MCP output budget of {max_output} bytes"
            ),
            "hint": "narrow the request (limit, root/depth); start the server with a larger --max-output for big results",
        },
        "truncated_output": true,
    });
    if let Some(id) = value.get("id") {
        error["id"] = id.clone();
    }
    to_string(&error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_lists_and_keeps_valid_json() {
        let items: Vec<_> = (0..100)
            .map(|i| json!({ "offset": i, "snippet": "x".repeat(50) }))
            .collect();
        let value = json!({ "ok": { "search": { "occurrences": { "total": 100, "truncated": false, "items": items } } } });
        let fitted = fit(value, 1500);
        assert!(!fitted.overflow);
        assert!(fitted.text.len() <= 1500);
        let parsed: Value = serde_json::from_str(&fitted.text).unwrap();
        assert_eq!(parsed["truncated_output"], true);
        assert_eq!(parsed["ok"]["search"]["occurrences"]["truncated"], true);
        let entry = &parsed["truncated_items"]["/ok/search/occurrences/items"];
        assert_eq!(entry["total"], 100);
        assert_eq!(
            entry["kept"].as_u64().unwrap() as usize,
            parsed["ok"]["search"]["occurrences"]["items"]
                .as_array()
                .unwrap()
                .len()
        );
    }

    #[test]
    fn unshrinkable_becomes_error() {
        let value =
            json!({ "id": 3, "ok": { "export": { "data": "y".repeat(500) } } });
        let fitted = fit(value, 100);
        assert!(fitted.overflow);
        let parsed: Value = serde_json::from_str(&fitted.text).unwrap();
        assert_eq!(parsed["error"]["kind"], "too_large");
        assert_eq!(parsed["id"], 3);
    }
}
