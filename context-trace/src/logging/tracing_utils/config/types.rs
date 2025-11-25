//! Configuration types for test tracing

use serde::{
    Deserialize,
    Serialize,
};

fn default_true() -> bool {
    true
}

/// Configuration for trait context display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraitContextConfig {
    /// Show the trait name (e.g., [trait: StateAdvance])
    #[serde(default = "default_true")]
    pub show_trait_name: bool,
    /// Show the implementing type (e.g., <ParentState>)
    #[serde(default = "default_true")]
    pub show_self_type: bool,
    /// Show associated types (e.g., [Next=RootChildState])
    #[serde(default = "default_true")]
    pub show_associated_types: bool,
}

impl Default for TraitContextConfig {
    fn default() -> Self {
        Self {
            show_trait_name: true,
            show_self_type: true,
            show_associated_types: true,
        }
    }
}

/// Configuration for span enter events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanEnterConfig {
    /// Show the span enter message
    #[serde(default = "default_true")]
    pub show: bool,
    /// Show function signatures for instrumented spans
    #[serde(default = "default_true")]
    pub show_fn_signature: bool,
    /// Show span fields when spans are entered
    #[serde(default = "default_true")]
    pub show_fields: bool,
    /// Configuration for trait context display
    #[serde(default)]
    pub trait_context: TraitContextConfig,
}

impl Default for SpanEnterConfig {
    fn default() -> Self {
        Self {
            show: true,
            show_fn_signature: true,
            show_fields: true,
            trait_context: TraitContextConfig::default(),
        }
    }
}

/// Configuration for span close events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanCloseConfig {
    /// Show the span close message at all
    #[serde(default = "default_true")]
    pub show: bool,
    /// Show time measurements when spans close
    #[serde(default = "default_true")]
    pub show_timing: bool,
}

impl Default for SpanCloseConfig {
    fn default() -> Self {
        Self {
            show: true,
            show_timing: true,
        }
    }
}

/// Configuration for panic message logging
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanicConfig {
    /// Show panic messages at all
    #[serde(default = "default_true")]
    pub show: bool,
    /// Show the panic message content
    #[serde(default = "default_true")]
    pub show_message: bool,
    /// Show stderr output (🔥 PANIC: ...)
    #[serde(default = "default_true")]
    pub show_stderr: bool,
    /// Call the default panic hook (which prints to stderr)
    /// Set to false to suppress stderr output and only log to file
    #[serde(default = "default_true")]
    pub show_default_hook: bool,
}

impl Default for PanicConfig {
    fn default() -> Self {
        Self {
            show: true,
            show_message: true,
            show_stderr: true,
            show_default_hook: true,
        }
    }
}

/// Configuration for whitespace and visual separation in logs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(Default)]
pub struct WhitespaceConfig {
    /// Add blank line before events
    #[serde(default)]
    pub blank_line_before_events: bool,
    /// Add blank line after events
    #[serde(default)]
    pub blank_line_after_events: bool,
    /// Add blank line before span enter
    #[serde(default)]
    pub blank_line_before_span_enter: bool,
    /// Add blank line after span enter
    #[serde(default)]
    pub blank_line_after_span_enter: bool,
    /// Add blank line before span close
    #[serde(default)]
    pub blank_line_before_span_close: bool,
    /// Add blank line after span close
    #[serde(default)]
    pub blank_line_after_span_close: bool,
}

/// Formatting options for log output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatConfig {
    /// Configuration for span enter events
    #[serde(default)]
    pub span_enter: SpanEnterConfig,
    /// Configuration for span close events
    #[serde(default)]
    pub span_close: SpanCloseConfig,
    /// Configuration for panic logging
    #[serde(default)]
    pub panic: PanicConfig,
    /// Configuration for whitespace and visual separation
    #[serde(default)]
    pub whitespace: WhitespaceConfig,
    /// Enable visual indentation with box-drawing characters
    #[serde(default = "default_true")]
    pub enable_indentation: bool,
    /// Show file location for events
    #[serde(default = "default_true")]
    pub show_file_location: bool,
    /// Enable ANSI colors in output
    #[serde(default = "default_true")]
    pub enable_ansi: bool,
    /// Show timestamp at the beginning of each log line
    #[serde(default = "default_true")]
    pub show_timestamp: bool,
    /// Enable logging to stdout (overrides LOG_STDOUT env var if present)
    #[serde(default)]
    pub log_to_stdout: Option<bool>,
    /// Log level filter (e.g., "debug", "trace", "context_search=trace,context_trace=debug")
    /// Overrides LOG_FILTER env var if present
    #[serde(default)]
    pub log_filter: Option<String>,
    /// Keep log files even when tests pass (overrides KEEP_LOGS env var if present)
    #[serde(default)]
    pub keep_logs: Option<bool>,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            span_enter: SpanEnterConfig::default(),
            span_close: SpanCloseConfig::default(),
            panic: PanicConfig::default(),
            whitespace: WhitespaceConfig::default(),
            enable_indentation: true,
            show_file_location: true,
            enable_ansi: true,
            show_timestamp: true,
            log_to_stdout: None,
            log_filter: None,
            keep_logs: None,
        }
    }
}
