//! Special field extraction for span instrumentation
//!
//! Captures special span fields (fn_sig, self_type, trait_name, *_type)
//! into extensions during span creation, enabling direct access in
//! format_event without parsing formatted strings.

use tracing::{
    Subscriber,
    field::{
        Field,
        Visit,
    },
};
use tracing_subscriber::{
    layer::{
        Context,
        Layer,
    },
    registry::LookupSpan,
};

/// Fields that are displayed specially in the formatter rather than as regular span fields.
///
/// These are captured during span creation by `SpecialFieldExtractor` and stored
/// in span extensions for direct access by the event formatter.
#[derive(Clone, Debug, Default)]
pub(super) struct SpecialFields {
    /// Function signature (from `fn_sig` field)
    pub fn_sig: Option<String>,
    /// Self type for trait implementations (from `self_type` field)
    pub self_type: Option<String>,
    /// Trait name (from `trait_name` field)
    pub trait_name: Option<String>,
    /// Associated type fields (e.g., `next_type` → ("next_type", "SomeType"))
    pub associated_types: Vec<(String, String)>,
}

impl SpecialFields {
    fn has_any(&self) -> bool {
        self.fn_sig.is_some()
            || self.self_type.is_some()
            || self.trait_name.is_some()
            || !self.associated_types.is_empty()
    }
}

/// Check if a field name is a special field that should be extracted separately.
///
/// Special fields are: `fn_sig`, `trait_name`, and anything ending with `_type`
/// (including `self_type` and associated type fields like `next_type`, `error_type`).
pub(super) fn is_special_field(name: &str) -> bool {
    name == "fn_sig" || name == "trait_name" || name.ends_with("_type")
}

/// Visitor that extracts special fields from span attributes.
#[derive(Default)]
struct SpecialFieldVisitor {
    fields: SpecialFields,
}

impl SpecialFieldVisitor {
    fn record_special(
        &mut self,
        name: &str,
        value: &str,
    ) {
        match name {
            "fn_sig" => self.fields.fn_sig = Some(value.to_string()),
            "self_type" => self.fields.self_type = Some(value.to_string()),
            "trait_name" => self.fields.trait_name = Some(value.to_string()),
            name if name.ends_with("_type") => {
                self.fields
                    .associated_types
                    .push((name.to_string(), value.to_string()));
            },
            _ => {},
        }
    }
}

impl Visit for SpecialFieldVisitor {
    fn record_debug(
        &mut self,
        field: &Field,
        value: &dyn std::fmt::Debug,
    ) {
        let value_str = format!("{:?}", value);
        let value_str = value_str.trim_matches('"');
        self.record_special(field.name(), value_str);
    }

    fn record_str(
        &mut self,
        field: &Field,
        value: &str,
    ) {
        self.record_special(field.name(), value);
    }
}

/// Layer that extracts special fields and stores them in span extensions.
///
/// This layer should be added BEFORE the fmt layer in the subscriber stack
/// so that special fields are available when `format_event` runs.
pub(super) struct SpecialFieldExtractor;

impl<S> Layer<S> for SpecialFieldExtractor
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            let mut visitor = SpecialFieldVisitor::default();
            attrs.record(&mut visitor);
            if visitor.fields.has_any() {
                span.extensions_mut().insert(visitor.fields);
            }
        }
    }
}
