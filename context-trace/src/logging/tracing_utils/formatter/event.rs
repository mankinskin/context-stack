//! Main event formatting logic (FormatEvent trait implementation)

use super::{
    core::CompactFieldsFormatter,
    field_visitor::FieldVisitor,
    fields::filter_span_fields,
    helpers::extract_trait_context,
    string_utils::strip_ansi_codes,
    syntax::{
        self,
        highlight_rust_signature,
    },
};

use std::fmt::{
    self,
    Write,
};
use tracing::{
    Event,
    Level,
    Subscriber,
};
use tracing_subscriber::{
    fmt::{
        FmtContext,
        FormatEvent,
        FormatFields,
        format,
    },
    registry::LookupSpan,
};

impl<S, N> FormatEvent<S, N> for CompactFieldsFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        // Get current span context for indentation
        let span_count =
            ctx.event_scope().map(|scope| scope.count()).unwrap_or(0);

        // Check if this is a span lifecycle event
        let mut message_text = String::new();
        event.record(&mut |field: &tracing::field::Field,
                           value: &dyn fmt::Debug| {
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
            "  "
        } else if self.config.enable_indentation {
            "\u{25cf} " // ● Regular event
        } else {
            "  "
        };

        // Create indentation with visual gutters
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
            let spaces = "  ".repeat(span_count);
            (spaces.clone(), spaces)
        };

        // Write indentation and timestamp
        write!(writer, "{}", indent)?;
        if self.config.show_timestamp {
            use tracing_subscriber::fmt::time::FormatTime;
            self.timer.format_time(&mut writer)?;
        }

        // Write level (use span's level for span events)
        let level = if is_span_event {
            ctx.event_scope()
                .and_then(|scope| scope.from_root().last())
                .map(|span| *span.metadata().level())
                .unwrap_or(*event.metadata().level())
        } else {
            *event.metadata().level()
        };

        format_level(&mut writer, level, self.config.enable_ansi)?;

        write!(writer, "  ")?;

        // Format the message content
        if is_span_event {
            format_span_event_message(
                &mut writer,
                ctx,
                &message_text,
                &self.config,
                &gutter_indent,
            )?;
        } else {
            write!(writer, "EVENT: {}", message_text.trim_matches('"'))?;
        }

        // Record non-message fields
        let mut visitor = FieldVisitor::new(
            &mut writer as &mut dyn Write,
            gutter_indent.clone(),
        );
        event.record(&mut visitor);
        visitor.result?;

        // For SPAN ENTERED events, display span's fields if enabled
        if self.config.span_enter.show_fields && is_span_enter {
            format_span_fields(&mut writer, ctx, &gutter_indent, &self.config)?;
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

/// Format level with optional ANSI colors
fn format_level(
    writer: &mut dyn Write,
    level: Level,
    enable_ansi: bool,
) -> fmt::Result {
    if enable_ansi {
        let level_str = match level {
            Level::ERROR => "\x1b[31mERROR\x1b[0m",
            Level::WARN => "\x1b[33m WARN\x1b[0m",
            Level::INFO => "\x1b[32m INFO\x1b[0m",
            Level::DEBUG => "\x1b[34mDEBUG\x1b[0m",
            Level::TRACE => "\x1b[35mTRACE\x1b[0m",
        };
        write!(writer, " {}", level_str)
    } else {
        write!(writer, " {:5}", level)
    }
}

/// Format span event message (enter/close)
fn format_span_event_message<S, N>(
    writer: &mut dyn Write,
    ctx: &FmtContext<'_, S, N>,
    message_text: &str,
    config: &super::config::FormatConfig,
    _gutter_indent: &str,
) -> fmt::Result
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    let span = ctx.event_scope().and_then(|scope| scope.from_root().last());
    let span_name = span.as_ref().map(|s| s.name()).unwrap_or("?");
    let target = span.as_ref().map(|s| s.metadata().target()).unwrap_or("");

    match message_text {
        "\"enter\"" => {
            let mut parts = Vec::new();
            parts.push(format!("SPAN ENTERED: {}::{}", target, span_name));

            // Get formatted fields (but don't display them inline - they'll be shown below)
            let fields_str_opt = span.as_ref().and_then(|s| {
                s.extensions()
                    .get::<tracing_subscriber::fmt::FormattedFields<N>>()
                    .map(|f| strip_ansi_codes(f.as_str()))
            });

            // Extract and display trait context
            if (config.span_enter.trait_context.show_trait_name
                || config.span_enter.trait_context.show_self_type
                || config.span_enter.trait_context.show_associated_types)
                && let Some(trait_ctx) = fields_str_opt
                    .as_ref()
                    .and_then(|fs| extract_trait_context(fs))
            {
                if config.span_enter.trait_context.show_trait_name
                    && let Some(trait_name) = trait_ctx.trait_name
                {
                    parts.push(format!("[trait: {}]", trait_name));
                }
                if config.span_enter.trait_context.show_self_type
                    && let Some(self_type) = trait_ctx.self_type
                {
                    let type_name =
                        self_type.rsplit("::").next().unwrap_or(&self_type);
                    parts.push(format!("<{}>", type_name));
                }
                if config.span_enter.trait_context.show_associated_types
                    && !trait_ctx.associated_types.is_empty()
                {
                    let assoc_str = trait_ctx
                        .associated_types
                        .iter()
                        .map(|(name, ty)| {
                            let ty_short = ty.rsplit("::").next().unwrap_or(ty);
                            format!("{}={}", name, ty_short)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    parts.push(format!("[{}]", assoc_str));
                }
            }

            // Extract and display function signature
            if config.span_enter.show_fn_signature
                && let Some(fn_sig) = extract_fn_sig(fields_str_opt.as_ref())
            {
                let highlighted =
                    highlight_rust_signature(&fn_sig, config.enable_ansi);
                parts.push(format!("- {}", highlighted));
            }

            write!(writer, "{}", parts.join(" "))
        },
        "\"close\"" => {
            write!(writer, "SPAN CLOSED: {}", span_name)?;
            if config.span_close.show_timing {
                // Timing will be shown by field visitor
            }
            Ok(())
        },
        _ => write!(writer, "{}", message_text.trim_matches('"')),
    }
}

/// Extract fn_sig from formatted fields
fn extract_fn_sig(fields_str_opt: Option<&String>) -> Option<String> {
    fields_str_opt.and_then(|fields_str| {
        // Pattern: fn_sig="..."
        if let Some(idx) = fields_str.find("fn_sig=\"") {
            let start = idx + 8;
            if let Some(end_offset) = fields_str[start..].find('"') {
                return Some(fields_str[start..start + end_offset].to_string());
            }
        }

        // Pattern: fn_sig=...
        if let Some(idx) = fields_str.find("fn_sig=") {
            let start = idx + 7;
            let remaining = &fields_str[start..];
            let end = remaining
                .find(char::is_whitespace)
                .unwrap_or(remaining.len());
            if end > 0 {
                return Some(remaining[..end].trim_matches('"').to_string());
            }
        }
        None
    })
}

/// Format span fields for SPAN ENTERED events
fn format_span_fields<S, N>(
    writer: &mut dyn Write,
    ctx: &FmtContext<'_, S, N>,
    gutter_indent: &str,
    config: &super::config::FormatConfig,
) -> fmt::Result
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    if let Some(span) =
        ctx.event_scope().and_then(|scope| scope.from_root().last())
        && let Some(fields) =
            span.extensions()
                .get::<tracing_subscriber::fmt::FormattedFields<N>>()
    {
        let fields_str = strip_ansi_codes(fields.as_str());

        // Filter out special fields that are displayed elsewhere
        let cleaned = filter_span_fields(&fields_str);

        // Also remove trait context fields if they were displayed
        let mut final_cleaned = cleaned;
        if config.span_enter.trait_context.show_self_type {
            final_cleaned =
                remove_all_occurrences(&final_cleaned, "self_type=");
        }
        if config.span_enter.trait_context.show_trait_name {
            final_cleaned =
                remove_all_occurrences(&final_cleaned, "trait_name=");
        }
        if config.span_enter.trait_context.show_associated_types {
            final_cleaned = remove_associated_types(&final_cleaned);
        }

        let trimmed = final_cleaned.trim();
        if !trimmed.is_empty() {
            for line in trimmed.lines() {
                write!(writer, "\n{}    {}", gutter_indent, line)?;
            }
        }
    }
    Ok(())
}

/// Remove all occurrences of a field from formatted fields string
fn remove_all_occurrences(
    fields: &str,
    field_prefix: &str,
) -> String {
    let mut result = fields.to_string();
    while let Some(start) = result.find(field_prefix) {
        let after_eq = start + field_prefix.len();
        let remaining = &result[after_eq..];
        let field_end = if let Some(stripped) = remaining.strip_prefix('"') {
            stripped.find('"').map(|i| after_eq + i + 2)
        } else {
            remaining.find(char::is_whitespace).map(|i| after_eq + i)
        }
        .unwrap_or(result.len());
        result.replace_range(start..field_end, "");
    }
    result
}

/// Remove associated type fields (ending with _type, except self_type/trait_name)
fn remove_associated_types(fields: &str) -> String {
    let mut result = fields.to_string();
    let mut pos = 0;
    while pos < result.len() {
        if let Some(type_pos) = result[pos..].find("_type=") {
            let abs_pos = pos + type_pos;
            let field_start = result[..abs_pos]
                .rfind(char::is_whitespace)
                .map(|i| i + 1)
                .unwrap_or(0);
            let field_name = &result[field_start..abs_pos];

            if field_name != "self" && field_name != "trait_name" {
                let after_eq = abs_pos + 6;
                let remaining = &result[after_eq..];
                let field_end = if let Some(stripped) =
                    remaining.strip_prefix('"')
                {
                    stripped.find('"').map(|i| after_eq + i + 2)
                } else {
                    remaining.find(char::is_whitespace).map(|i| after_eq + i)
                }
                .unwrap_or(result.len());
                result.replace_range(field_start..field_end, "");
                pos = field_start;
            } else {
                pos = abs_pos + 1;
            }
        } else {
            break;
        }
    }
    result
}
