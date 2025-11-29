//! Custom span field formatter that respects Display formatting

use super::field_visitor::FieldVisitor;
use std::fmt;
use tracing_subscriber::{
    field::RecordFields,
    fmt::{
        FormatFields,
        format::Writer,
    },
};

/// Custom span field formatter that uses our FieldVisitor
/// This respects Display formatting (%) vs Debug formatting (?)
pub struct SpanFieldFormatter;

impl<'writer> FormatFields<'writer> for SpanFieldFormatter {
    fn format_fields<R: RecordFields>(
        &self,
        mut writer: Writer<'writer>,
        fields: R,
    ) -> fmt::Result {
        let mut buf = String::new();
        let mut visitor = FieldVisitor::new(&mut buf, String::new());
        fields.record(&mut visitor);
        visitor.result?;
        write!(writer, "{}", buf)
    }
}
