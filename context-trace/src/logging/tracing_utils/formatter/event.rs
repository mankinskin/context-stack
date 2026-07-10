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

enum SpanEventKind {
    Enter,
    Close,
}

struct EventRendering {
    message_text: String,
    span_event: Option<SpanEventKind>,
    gutter_indent: String,
    indent: String,
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
        event: &Event<'_>,
    ) -> fmt::Result {
        let rendering = build_event_rendering(self, ctx, event);
        if should_skip_event(self, &rendering) {
            return Ok(());
        }

        if should_add_blank_before(self, rendering.span_event.as_ref()) {
            writeln!(writer)?;
        }

        write!(writer, "{}", rendering.indent)?;
        if self.config.show_timestamp {
            use tracing_subscriber::fmt::time::FormatTime;
            self.timer.format_time(&mut writer)?;
        }

        format_level(
            &mut writer,
            event_level(ctx, event, rendering.span_event.as_ref()),
            self.config.enable_ansi,
        )?;
        write!(writer, "  ")?;

        if rendering.span_event.is_some() {
            format_span_event_message(
                &mut writer,
                ctx,
                &rendering.message_text,
                &self.config,
                &rendering.gutter_indent,
            )?;
        } else {
            write!(
                writer,
                "EVENT: {}",
                rendering.message_text.trim_matches('"')
            )?;
        }

        let mut visitor = FieldVisitor::new(
            &mut writer as &mut dyn Write,
            rendering.gutter_indent.clone(),
        );
        event.record(&mut visitor);
        visitor.result?;

        if should_format_span_fields(
            &self.config,
            rendering.span_event.as_ref(),
        ) {
            format_span_fields(
                &mut writer,
                ctx,
                &rendering.gutter_indent,
                &self.config,
            )?;
        }

        write_file_location_if_enabled(
            &mut writer,
            event,
            &rendering.gutter_indent,
            self.config.show_file_location,
        )?;
        writeln!(writer)?;

        if should_add_blank_after(self, rendering.span_event.as_ref()) {
            writeln!(writer)?;
        }

        Ok(())
    }
}

fn build_event_rendering<S, N>(
    formatter: &CompactFieldsFormatter,
    ctx: &FmtContext<'_, S, N>,
    event: &Event<'_>,
) -> EventRendering
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    let message_text = event_message_text(event);
    let span_count = ctx.event_scope().map(|scope| scope.count()).unwrap_or(0);
    let span_event = span_event_kind(&message_text);
    let (gutter_indent, indent) = build_event_indentation(
        formatter.config.enable_indentation,
        span_count,
        span_event.as_ref(),
    );

    EventRendering {
        message_text,
        span_event,
        gutter_indent,
        indent,
    }
}

fn event_message_text(event: &Event<'_>) -> String {
    let mut message_text = String::new();
    event.record(&mut |field: &tracing::field::Field,
                       value: &dyn fmt::Debug| {
        if field.name() == "message" {
            message_text = format!("{:?}", value);
        }
    });
    message_text
}

fn span_event_kind(message_text: &str) -> Option<SpanEventKind> {
    match message_text {
        "\"enter\"" => Some(SpanEventKind::Enter),
        "\"close\"" => Some(SpanEventKind::Close),
        _ => None,
    }
}

fn is_hidden_span_lifecycle_event(message_text: &str) -> bool {
    matches!(message_text, "\"new\"" | "\"exit\"")
}

fn should_skip_event(
    formatter: &CompactFieldsFormatter,
    rendering: &EventRendering,
) -> bool {
    if is_hidden_span_lifecycle_event(&rendering.message_text) {
        return true;
    }

    match rendering.span_event.as_ref() {
        Some(SpanEventKind::Enter) => !formatter.config.span_enter.show,
        Some(SpanEventKind::Close) => !formatter.config.span_close.show,
        None => false,
    }
}

fn should_add_blank_before(
    formatter: &CompactFieldsFormatter,
    span_event: Option<&SpanEventKind>,
) -> bool {
    match span_event {
        Some(SpanEventKind::Enter) =>
            formatter.config.whitespace.blank_line_before_span_enter,
        Some(SpanEventKind::Close) =>
            formatter.config.whitespace.blank_line_before_span_close,
        None => formatter.config.whitespace.blank_line_before_events,
    }
}

fn should_add_blank_after(
    formatter: &CompactFieldsFormatter,
    span_event: Option<&SpanEventKind>,
) -> bool {
    match span_event {
        Some(SpanEventKind::Enter) =>
            formatter.config.whitespace.blank_line_after_span_enter,
        Some(SpanEventKind::Close) =>
            formatter.config.whitespace.blank_line_after_span_close,
        None => formatter.config.whitespace.blank_line_after_events,
    }
}

fn event_decoration(
    enable_indentation: bool,
    span_event: Option<&SpanEventKind>,
) -> &'static str {
    match (enable_indentation, span_event) {
        (true, Some(SpanEventKind::Enter)) => "\u{252c}\u{2500}",
        (true, Some(SpanEventKind::Close)) => "\u{2514}\u{2500}",
        (true, None) => "\u{25cf} ",
        (false, _) => "  ",
    }
}

fn build_event_indentation(
    enable_indentation: bool,
    span_count: usize,
    span_event: Option<&SpanEventKind>,
) -> (String, String) {
    let decoration = event_decoration(enable_indentation, span_event);
    if enable_indentation {
        let gutter_indent = if span_count > 0 {
            "│ ".repeat(span_count)
        } else {
            String::new()
        };

        let indent = if span_count > 0 {
            let parent_gutters = "│ ".repeat(span_count - 1);
            format!("{}{}", parent_gutters, decoration)
        } else {
            decoration.to_string()
        };
        (gutter_indent, indent)
    } else {
        let spaces = "  ".repeat(span_count);
        (spaces.clone(), spaces)
    }
}

fn event_level<S, N>(
    ctx: &FmtContext<'_, S, N>,
    event: &Event<'_>,
    span_event: Option<&SpanEventKind>,
) -> Level
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    if span_event.is_some() {
        ctx.event_scope()
            .and_then(|scope| scope.from_root().last())
            .map(|span| *span.metadata().level())
            .unwrap_or(*event.metadata().level())
    } else {
        *event.metadata().level()
    }
}

fn should_format_span_fields(
    config: &super::config::FormatConfig,
    span_event: Option<&SpanEventKind>,
) -> bool {
    config.span_enter.show_fields
        && matches!(span_event, Some(SpanEventKind::Enter))
}

fn write_file_location_if_enabled(
    writer: &mut dyn Write,
    event: &Event<'_>,
    gutter_indent: &str,
    show_file_location: bool,
) -> fmt::Result {
    if show_file_location
        && let Some(file) = event.metadata().file()
        && let Some(line) = event.metadata().line()
    {
        write!(writer, "\n{}    at {}:{}", gutter_indent, file, line)?;
    }
    Ok(())
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
        "\"enter\"" => write!(
            writer,
            "{}",
            format_span_enter_message(span, target, span_name, config)
        ),
        "\"close\"" => format_span_close_message(writer, span_name, config),
        _ => write!(writer, "{}", message_text.trim_matches('"')),
    }
}

fn format_span_enter_message<S>(
    span: Option<tracing_subscriber::registry::SpanRef<'_, S>>,
    target: &str,
    span_name: &str,
    config: &super::config::FormatConfig,
) -> String
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let special = span
        .as_ref()
        .and_then(|span| span.extensions().get::<SpecialFields>().cloned());
    let mut parts = vec![format!("SPAN ENTERED: {}::{}", target, span_name)];

    append_trait_context_parts(&mut parts, special.as_ref(), config);
    append_signature_part(&mut parts, special.as_ref(), config);

    parts.join(" ")
}

fn append_trait_context_parts(
    parts: &mut Vec<String>,
    special: Option<&SpecialFields>,
    config: &super::config::FormatConfig,
) {
    let Some(special) = special else {
        return;
    };

    if config.span_enter.trait_context.show_trait_name
        && let Some(trait_name) = special.trait_name.as_ref()
    {
        parts.push(format!("[trait: {}]", trait_name));
    }
    if config.span_enter.trait_context.show_self_type
        && let Some(self_type) = special.self_type.as_ref()
    {
        let type_name = self_type.rsplit("::").next().unwrap_or(self_type);
        parts.push(format!("<{}>", type_name));
    }
    if config.span_enter.trait_context.show_associated_types
        && !special.associated_types.is_empty()
    {
        parts.push(format!("[{}]", format_associated_types(special)));
    }
}

fn append_signature_part(
    parts: &mut Vec<String>,
    special: Option<&SpecialFields>,
    config: &super::config::FormatConfig,
) {
    if config.span_enter.show_fn_signature
        && let Some(fn_sig) =
            special.and_then(|special| special.fn_sig.as_ref())
    {
        let highlighted = highlight_rust_signature(fn_sig, config.enable_ansi);
        parts.push(format!("- {}", highlighted));
    }
}

fn format_associated_types(special: &SpecialFields) -> String {
    special
        .associated_types
        .iter()
        .map(|(name, ty)| {
            let assoc_name = format_associated_type_name(name);
            let ty_short = ty.rsplit("::").next().unwrap_or(ty);
            format!("{}={}", assoc_name, ty_short)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_associated_type_name(name: &str) -> String {
    name.strip_suffix("_type")
        .map(capitalize_first)
        .unwrap_or_else(|| name.to_string())
}

fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

fn format_span_close_message(
    writer: &mut dyn Write,
    span_name: &str,
    config: &super::config::FormatConfig,
) -> fmt::Result {
    write!(writer, "SPAN CLOSED: {}", span_name)?;
    if config.span_close.show_timing {}
    Ok(())
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
