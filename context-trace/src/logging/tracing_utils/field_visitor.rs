//! Field formatting utilities for events and spans

use tracing::field::{
    Field,
    Visit,
};

/// Custom field visitor that formats each field on its own line
pub(super) struct FieldVisitor<'a> {
    pub(super) writer: &'a mut dyn std::fmt::Write,
    pub(super) indent: String,
    pub(super) result: std::fmt::Result,
}

impl<'a> FieldVisitor<'a> {
    pub(super) fn new(
        writer: &'a mut dyn std::fmt::Write,
        indent: String,
    ) -> Self {
        Self {
            writer,
            indent,
            result: Ok(()),
        }
    }

    fn record_field(
        &mut self,
        field: &Field,
        value: &dyn std::fmt::Debug,
    ) {
        // Skip the message field - it's already been written
        if field.name() == "message" {
            return;
        }

        if self.result.is_err() {
            return;
        }

        // Format the value into a string first using alternate Debug (pretty print)
        let value_str = format!("{:#?}", value);

        // Calculate the base indentation for this field
        // This is where the field key starts
        let field_indent = format!("{}    ", self.indent);

        // Write field name on new line
        self.result =
            write!(self.writer, "\n{}{}=", field_indent, field.name());
        if self.result.is_err() {
            return;
        }

        // All lines of the value should be indented relative to where the field key is
        // The value's own Debug formatting provides nesting structure,
        // we just shift it all by the field's indentation
        let mut first_line = true;
        for line in value_str.lines() {
            if first_line {
                // First line goes right after the '='
                self.result = write!(self.writer, "{}", line);
                first_line = false;
            } else {
                // Subsequent lines maintain the Debug output's indentation
                // but shifted by the field's base indentation
                self.result = write!(self.writer, "\n{}{}", field_indent, line);
            }
            if self.result.is_err() {
                return;
            }
        }
    }
}

impl<'a> Visit for FieldVisitor<'a> {
    fn record_debug(
        &mut self,
        field: &Field,
        value: &dyn std::fmt::Debug,
    ) {
        // For Debug format (?), use standard pretty-print Debug
        self.record_field(field, value);
    }

    // Override record_str to handle Display values
    // This is called when using % format specifier in tracing macros
    fn record_str(
        &mut self,
        field: &Field,
        value: &str,
    ) {
        // Skip the message field
        if field.name() == "message" {
            return;
        }

        if self.result.is_err() {
            return;
        }

        let field_indent = format!("{}    ", self.indent);

        // Write field name on new line
        self.result =
            write!(self.writer, "\n{}{}=", field_indent, field.name());
        if self.result.is_err() {
            return;
        }

        // For Display string values (from %), indent all lines appropriately
        let mut first_line = true;
        for line in value.lines() {
            if first_line {
                self.result = write!(self.writer, "{}", line);
                first_line = false;
            } else {
                self.result = write!(self.writer, "\n{}{}", field_indent, line);
            }
            if self.result.is_err() {
                return;
            }
        }
    }
}
