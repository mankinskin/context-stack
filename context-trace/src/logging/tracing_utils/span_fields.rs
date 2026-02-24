//! Custom span field formatter that respects Display formatting
//!
//! Filters out special fields (fn_sig, self_type, trait_name, *_type)
//! which are captured separately by `SpecialFieldExtractor`.

use super::{
    field_visitor::FieldVisitor,
    special_fields::is_special_field,
};
use std::fmt;
use tracing::field::{
    Field,
    Visit,
};
use tracing_subscriber::{
    field::RecordFields,
    fmt::{
        FormatFields,
        format::Writer,
    },
};

/// Custom span field formatter that uses our FieldVisitor.
///
/// This respects Display formatting (%) vs Debug formatting (?)
/// and filters out special fields that are displayed separately
/// by the event formatter.
pub struct SpanFieldFormatter;

impl<'writer> FormatFields<'writer> for SpanFieldFormatter {
    fn format_fields<R: RecordFields>(
        &self,
        mut writer: Writer<'writer>,
        fields: R,
    ) -> fmt::Result {
        let mut buf = String::new();
        let mut visitor = FilteringFieldVisitor {
            inner: FieldVisitor::new(&mut buf, String::new()),
        };
        fields.record(&mut visitor);
        visitor.inner.result?;
        write!(writer, "{}", buf)
    }
}

/// Field visitor that wraps FieldVisitor and skips special fields.
struct FilteringFieldVisitor<'a> {
    inner: FieldVisitor<'a>,
}

impl<'a> Visit for FilteringFieldVisitor<'a> {
    fn record_debug(
        &mut self,
        field: &Field,
        value: &dyn std::fmt::Debug,
    ) {
        if !is_special_field(field.name()) {
            self.inner.record_debug(field, value);
        }
    }

    fn record_str(
        &mut self,
        field: &Field,
        value: &str,
    ) {
        if !is_special_field(field.name()) {
            self.inner.record_str(field, value);
        }
    }
}
