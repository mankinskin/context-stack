//! Event formatter for compact log output

use super::{
    config::FormatConfig,
    field_visitor::FieldVisitor,
    string_utils::strip_ansi_codes,
    syntax::highlight_rust_signature,
    timer::CompactTimer,
};
use tracing::{
    Level,
    Subscriber,
    field::Field,
};
use tracing_subscriber::{
    fmt::{
        FmtContext,
        FormatEvent,
        FormatFields,
        format,
        time::FormatTime,
    },
    registry::LookupSpan,
};

/// Custom event formatter that puts each field on its own line,
/// with multi-line values indented on subsequent lines.
pub(super) struct CompactFieldsFormatter {
    pub(super) timer: CompactTimer,
    pub(super) config: FormatConfig,
}

impl CompactFieldsFormatter {
    pub(super) fn new(config: FormatConfig) -> Self {
        Self {
            timer: CompactTimer::new(),
            config,
        }
    }
}

impl<S, N> FormatEvent<S, N> for CompactFieldsFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        // Get current span context for indentation
        let span_count =
            ctx.event_scope().map(|scope| scope.count()).unwrap_or(0);

        // Check if this is a span lifecycle event
        let mut message_text = String::new();
        event.record(&mut |field: &Field, value: &dyn std::fmt::Debug| {
            if field.name() == "message" {
                message_text = format!("{:?}", value);
            }
        });

        let is_span_event = message_text == "\"new\""
            || message_text == "\"enter\""
            || message_text == "\"exit\""
            || message_text == "\"close\"";

        let is_span_enter = message_text == "\"enter\"";
        let is_span_close = message_text == "\"close\"";

        // Skip redundant span events - only show enter and close
        // Also skip based on configuration
        let should_skip = message_text == "\"new\""
            || message_text == "\"exit\""
            || (is_span_enter && !self.config.span_enter.show)
            || (is_span_close && !self.config.span_close.show);
        if should_skip {
            return Ok(());
        }

        // Add blank line before event if configured
        let should_add_blank_before = if is_span_enter {
            self.config.whitespace.blank_line_before_span_enter
        } else if is_span_close {
            self.config.whitespace.blank_line_before_span_close
        } else {
            self.config.whitespace.blank_line_before_events
        };

        if should_add_blank_before {
            writeln!(writer)?;
        }

        // Determine decoration based on event type
        let decoration = if is_span_event && self.config.enable_indentation {
            match message_text.as_str() {
                "\"enter\"" => "\u{252c}\u{2500}", // ┬─ Opening span
                "\"close\"" => "\u{2514}\u{2500}", // └─ Closing span
                _ => "  ",
            }
        } else if is_span_event {
            "  " // No decoration if indentation disabled
        } else if self.config.enable_indentation {
            "\u{25cf} " // ● Regular event (filled circle)
        } else {
            "  "
        };

        // Create indentation with visual gutters, replacing last gutter with decoration
        let (gutter_indent, indent) = if self.config.enable_indentation {
            let gutter = if span_count > 0 {
                "│ ".repeat(span_count)
            } else {
                String::new()
            };

            let indent_str = if span_count > 0 {
                let parent_gutters = "│ ".repeat(span_count - 1);
                format!("{}{}", parent_gutters, decoration)
            } else {
                decoration.to_string()
            };
            (gutter, indent_str)
        } else {
            // No indentation - just use spaces
            let spaces = "  ".repeat(span_count);
            (spaces.clone(), spaces)
        };

        // Write indentation and timestamp
        write!(writer, "{}", indent)?;
        if self.config.show_timestamp {
            self.timer.format_time(&mut writer)?;
        }

        // Write level
        // For span events, use the span's level instead of the event's level
        let level = if is_span_event {
            ctx.event_scope()
                .and_then(|scope| scope.from_root().last())
                .map(|span| *span.metadata().level())
                .unwrap_or(*event.metadata().level())
        } else {
            *event.metadata().level()
        };
        
        if self.config.enable_ansi {
            let level_str = match level {
                Level::ERROR => "\x1b[31mERROR\x1b[0m",
                Level::WARN => "\x1b[33m WARN\x1b[0m",
                Level::INFO => "\x1b[32m INFO\x1b[0m",
                Level::DEBUG => "\x1b[34mDEBUG\x1b[0m",
                Level::TRACE => "\x1b[35mTRACE\x1b[0m",
            };
            write!(writer, " {}", level_str)?;
        } else {
            write!(writer, " {:5}", level)?;
        }

        // Write the message - separate handling for message field and other fields
        write!(writer, "  ")?;

        // For span events, write custom messages instead of quoted strings
        if is_span_event {
            let span =
                ctx.event_scope().and_then(|scope| scope.from_root().last());

            let span_name = span.as_ref().map(|s| s.name()).unwrap_or("?");
            let target =
                span.as_ref().map(|s| s.metadata().target()).unwrap_or("");

            let event_name = match message_text.as_str() {
                "\"new\"" => format!("SPAN CREATED: {}", span_name),
                "\"enter\"" => {
                    let mut parts = Vec::new();
                    let fields_str_opt = span.as_ref().and_then(|s| {
                        s.extensions()
                            .get::<tracing_subscriber::fmt::FormattedFields<N>>(
                            )
                            .map(|f| {
                                strip_ansi_codes(f.as_str()).replace('\n', " ")
                            })
                    });

                    // Extract trait context if any component is enabled
                    let trait_context = if self
                        .config
                        .span_enter
                        .trait_context
                        .show_trait_name
                        || self.config.span_enter.trait_context.show_self_type
                        || self
                            .config
                            .span_enter
                            .trait_context
                            .show_associated_types
                    {
                        fields_str_opt
                            .as_ref()
                            .and_then(|fs| extract_trait_context(fs))
                    } else {
                        None
                    };

                    // Try to extract fn_sig from the span's formatted fields if enabled
                    let fn_sig = if self.config.span_enter.show_fn_signature {
                        fields_str_opt.as_ref().and_then(|fields_str| {
                            // Parse fn_sig from formatted fields (format: fn_sig="...")
                            if let Some(idx) = fields_str.find("fn_sig=\"") {
                                let start = idx + 8; // Skip 'fn_sig="'
                                if let Some(end_offset) =
                                    fields_str[start..].find('"')
                                {
                                    return Some(
                                        fields_str[start..start + end_offset]
                                            .to_string(),
                                    );
                                }
                            }

                            // Also try without quotes (format: fn_sig=...)
                            if let Some(idx) = fields_str.find("fn_sig=") {
                                let start = idx + 7; // Skip 'fn_sig='
                                let remaining = &fields_str[start..];
                                let end = remaining
                                    .find(char::is_whitespace)
                                    .unwrap_or(remaining.len());
                                if end > 0 {
                                    let value = &remaining[..end];
                                    return Some(
                                        value.trim_matches('"').to_string(),
                                    );
                                }
                            }
                            None
                        })
                    } else {
                        None
                    };

                    // Build the message parts
                    parts.push(format!(
                        "SPAN ENTERED: {}::{}",
                        target, span_name
                    ));

                    // Add trait context if available
                    if let Some(ctx) = trait_context {
                        if self.config.span_enter.trait_context.show_trait_name
                            && let Some(trait_name) = ctx.trait_name
                        {
                            parts.push(format!("[trait: {}]", trait_name));
                        }
                        if self.config.span_enter.trait_context.show_self_type
                            && let Some(self_type) = ctx.self_type
                        {
                            // Extract just the type name from the full path
                            let type_name = self_type
                                .rsplit("::")
                                .next()
                                .unwrap_or(&self_type);
                            parts.push(format!("<{}>", type_name));
                        }
                        if self
                            .config
                            .span_enter
                            .trait_context
                            .show_associated_types
                            && !ctx.associated_types.is_empty()
                        {
                            let assoc_str = ctx
                                .associated_types
                                .iter()
                                .map(|(name, ty)| {
                                    let ty_short =
                                        ty.rsplit("::").next().unwrap_or(ty);
                                    format!("{}={}", name, ty_short)
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            parts.push(format!("[{}]", assoc_str));
                        }
                    }

                    // Add signature if available
                    if let Some(sig) = fn_sig {
                        let highlighted = highlight_rust_signature(
                            &sig,
                            self.config.enable_ansi,
                        );
                        parts.push(format!("- {}", highlighted));
                    }

                    parts.join(" ")
                },
                "\"exit\"" => format!("SPAN EXITED: {}", span_name),
                "\"close\"" => {
                    // Show timing info if enabled
                    if self.config.span_close.show_timing {
                        // The timing fields will be shown by the field visitor below
                        format!("SPAN CLOSED: {}", span_name)
                    } else {
                        format!("SPAN CLOSED: {}", span_name)
                    }
                },
                _ => message_text.clone(),
            };
            write!(writer, "{}", event_name)?;
        } else {
            // Regular event - write "EVENT:" prefix for clarity
            write!(writer, "EVENT: {}", message_text.trim_matches('"'))?;
        }

        // Then write all non-message fields on separate lines
        let mut visitor = FieldVisitor::new(
            &mut writer as &mut dyn std::fmt::Write,
            gutter_indent.clone(),
        );
        event.record(&mut visitor);
        visitor.result?;

        // For SPAN ENTERED events, also display the span's fields if enabled
        if self.config.span_enter.show_fields
            && message_text == "\"enter\""
            && let Some(span) =
                ctx.event_scope().and_then(|scope| scope.from_root().last())
            && let Some(fields) =
                span.extensions()
                    .get::<tracing_subscriber::fmt::FormattedFields<N>>()
        {
            // Display span fields directly without parsing
            let fields_str = strip_ansi_codes(fields.as_str());

            // Remove fn_sig and message fields if present by simple string replacement
            // This is hacky but avoids complex parsing
            let mut cleaned = fields_str.clone();

            // Try to remove fn_sig field (pattern: fn_sig="..." or fn_sig=value)
            if let Some(start) = cleaned.find("fn_sig=") {
                // Find the end of this field (next space or end of string)
                let after_eq = start + 7;
                let remaining = &cleaned[after_eq..];

                if let Some(stripped) = remaining.strip_prefix('"') {
                    // Quoted value - find closing quote
                    if let Some(quote_end) = stripped.find('"') {
                        let field_end = after_eq + quote_end + 2;
                        cleaned.replace_range(start..field_end, "");
                    }
                } else {
                    // Unquoted value - find next whitespace or end
                    let field_end = remaining
                        .find(char::is_whitespace)
                        .map(|i| after_eq + i)
                        .unwrap_or(cleaned.len());
                    cleaned.replace_range(start..field_end, "");
                }
            }

            // Remove message field similarly
            if let Some(start) = cleaned.find("message=") {
                let after_eq = start + 8;
                let remaining = &cleaned[after_eq..];

                if let Some(stripped) = remaining.strip_prefix('"') {
                    if let Some(quote_end) = stripped.find('"') {
                        let field_end = after_eq + quote_end + 2;
                        cleaned.replace_range(start..field_end, "");
                    }
                } else {
                    let field_end = remaining
                        .find(char::is_whitespace)
                        .map(|i| after_eq + i)
                        .unwrap_or(cleaned.len());
                    cleaned.replace_range(start..field_end, "");
                }
            }

            // Remove trait context fields if they were displayed inline
            // Remove self_type if it's being shown in trait context
            if self.config.span_enter.trait_context.show_self_type {
                while let Some(start) = cleaned.find("self_type=") {
                    let after_eq = start + 10;
                    let remaining = &cleaned[after_eq..];
                    let field_end =
                        if let Some(stripped) = remaining.strip_prefix('"') {
                            stripped.find('"').map(|i| after_eq + i + 2)
                        } else {
                            remaining
                                .find(char::is_whitespace)
                                .map(|i| after_eq + i)
                        }
                        .unwrap_or(cleaned.len());
                    cleaned.replace_range(start..field_end, "");
                }
            }

            // Remove trait_name if it's being shown in trait context
            if self.config.span_enter.trait_context.show_trait_name {
                while let Some(start) = cleaned.find("trait_name=") {
                    let after_eq = start + 11;
                    let remaining = &cleaned[after_eq..];
                    let field_end =
                        if let Some(stripped) = remaining.strip_prefix('"') {
                            stripped.find('"').map(|i| after_eq + i + 2)
                        } else {
                            remaining
                                .find(char::is_whitespace)
                                .map(|i| after_eq + i)
                        }
                        .unwrap_or(cleaned.len());
                    cleaned.replace_range(start..field_end, "");
                }
            }

            // Remove *_type fields (associated types) if they're being shown in trait context
            if self.config.span_enter.trait_context.show_associated_types {
                let mut pos = 0;
                while pos < cleaned.len() {
                    if let Some(type_pos) = cleaned[pos..].find("_type=") {
                        let abs_pos = pos + type_pos;
                        // Find start of field name (back to previous whitespace or start)
                        let field_start = cleaned[..abs_pos]
                            .rfind(char::is_whitespace)
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        let field_name = &cleaned[field_start..abs_pos];

                        // Skip if it's self_type or trait_name (already handled)
                        if field_name != "self" && field_name != "trait_name" {
                            let after_eq = abs_pos + 6; // Skip '_type='
                            let remaining = &cleaned[after_eq..];
                            let field_end = if let Some(stripped) =
                                remaining.strip_prefix('"')
                            {
                                stripped.find('"').map(|i| after_eq + i + 2)
                            } else {
                                remaining
                                    .find(char::is_whitespace)
                                    .map(|i| after_eq + i)
                            }
                            .unwrap_or(cleaned.len());
                            cleaned.replace_range(field_start..field_end, "");
                            pos = field_start;
                        } else {
                            pos = abs_pos + 1;
                        }
                    } else {
                        break;
                    }
                }
            }

            // Trim and display the remaining fields with proper indentation
            let trimmed = cleaned.trim();
            if !trimmed.is_empty() {
                // Split by lines and add gutter indentation to each line
                for line in trimmed.lines() {
                    write!(writer, "\n{}    {}", gutter_indent, line)?;
                }
            }
        }

        // For SPAN CLOSED events, filter timing fields if timing is disabled
        if message_text == "\"close\"" && !self.config.span_close.show_timing {
            // Note: timing fields (time.busy, time.idle) are still in the visitor output
            // To fully suppress them, we'd need to filter in the visitor
        }

        // Write file location if enabled
        if self.config.show_file_location
            && let Some(file) = event.metadata().file()
            && let Some(line) = event.metadata().line()
        {
            write!(writer, "\n{}    at {}:{}", gutter_indent, file, line)?;
        }

        writeln!(writer)?;

        // Add blank line after event if configured
        let should_add_blank_after = if is_span_enter {
            self.config.whitespace.blank_line_after_span_enter
        } else if is_span_close {
            self.config.whitespace.blank_line_after_span_close
        } else {
            self.config.whitespace.blank_line_after_events
        };

        if should_add_blank_after {
            writeln!(writer)?;
        }

        Ok(())
    }
}

/// Trait context extracted from span fields
#[derive(Debug)]
struct TraitContext {
    trait_name: Option<String>,
    self_type: Option<String>,
    associated_types: Vec<(String, String)>,
}

/// Extract trait context from formatted fields string
/// Looks for special fields: self_type, trait_name, and patterns like next_type, error_type, etc.
fn extract_trait_context(fields_str: &str) -> Option<TraitContext> {
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
