//! Main event formatting logic (FormatEvent trait implementation)

use super::{
    core::CompactFieldsFormatter,
    field_visitor::FieldVisitor,
    special_fields::SpecialFields,
    syntax::highlight_rust_signature,
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

            // Read special fields directly from span extensions
            let special = span
                .as_ref()
                .and_then(|s| s.extensions().get::<SpecialFields>().cloned());

            // Display trait context from special fields
            if let Some(ref special) = special {
                if config.span_enter.trait_context.show_trait_name
                    && let Some(ref trait_name) = special.trait_name
                {
                    parts.push(format!("[trait: {}]", trait_name));
                }
                if config.span_enter.trait_context.show_self_type
                    && let Some(ref self_type) = special.self_type
                {
                    let type_name =
                        self_type.rsplit("::").next().unwrap_or(self_type);
                    parts.push(format!("<{}>", type_name));
                }
                if config.span_enter.trait_context.show_associated_types
                    && !special.associated_types.is_empty()
                {
                    let assoc_str = special
                        .associated_types
                        .iter()
                        .map(|(name, ty)| {
                            let assoc_name = name
                                .strip_suffix("_type")
                                .map(|s| {
                                    let mut chars = s.chars();
                                    match chars.next() {
                                        None => String::new(),
                                        Some(first) => first
                                            .to_uppercase()
                                            .chain(chars)
                                            .collect(),
                                    }
                                })
                                .unwrap_or_else(|| name.clone());
                            let ty_short = ty.rsplit("::").next().unwrap_or(ty);
                            format!("{}={}", assoc_name, ty_short)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    parts.push(format!("[{}]", assoc_str));
                }
            }

            // Display function signature from special fields
            if config.span_enter.show_fn_signature
                && let Some(ref fn_sig) =
                    special.as_ref().and_then(|s| s.fn_sig.as_ref())
            {
                let highlighted =
                    highlight_rust_signature(fn_sig, config.enable_ansi);
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

/// Format span fields for SPAN ENTERED events
///
/// Special fields (fn_sig, self_type, trait_name, *_type) are already
/// filtered out by SpanFieldFormatter, so we just display what remains.
fn format_span_fields<S, N>(
    writer: &mut dyn Write,
    ctx: &FmtContext<'_, S, N>,
    gutter_indent: &str,
    _config: &super::config::FormatConfig,
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
        let fields_str = fields.as_str();
        let trimmed = fields_str.trim_matches('\n');
        for line in trimmed.lines() {
            if !line.trim().is_empty() {
                write!(writer, "\n{}{}", gutter_indent, line)?;
            }
        }
    }
    Ok(())
}
