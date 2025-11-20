//! Configuration for test tracing

use serde::{
    Deserialize,
    Serialize,
};
use std::{
    env,
    fs,
    path::{
        Path,
        PathBuf,
    },
};
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;

use super::path::{
    get_target_dir,
    get_workspace_root,
};

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

fn default_true() -> bool {
    true
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
}

impl Default for PanicConfig {
    fn default() -> Self {
        Self {
            show: true,
            show_message: true,
            show_stderr: true,
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
        }
    }
}

impl FormatConfig {
    /// Load configuration from a TOML file
    ///
    /// Example file format:
    /// ```toml
    /// [span_enter]
    /// show = true
    /// show_fn_signature = true
    /// show_fields = true
    /// show_trait_context = true
    ///
    /// [span_close]
    /// show = true
    /// show_timing = true
    ///
    /// [whitespace]
    /// blank_line_before_events = false
    /// blank_line_after_events = false
    /// blank_line_before_span_enter = false
    /// blank_line_after_span_enter = false
    /// blank_line_before_span_close = false
    /// blank_line_after_span_close = false
    ///
    /// enable_indentation = true
    /// show_file_location = true
    /// enable_ansi = true
    /// show_timestamp = true
    ///
    /// # Optional: override environment variables
    /// log_to_stdout = true
    /// log_filter = "debug"
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let contents = fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        toml::from_str(&contents)
            .map_err(|e| {
                let msg = format!("Failed to parse config file: {}", e);
                if msg.contains("unknown field") {
                    eprintln!("\n[tracing.toml] ERROR: {}\n  -> Check for typos or misplaced options (global options must be before any [section])\n", msg);
                }
                msg
            })
    }

    /// Try to load from a config file, falling back to environment variables, then defaults
    ///
    /// Searches for config files in this order:
    /// 1. Path specified in TRACING_CONFIG environment variable
    /// 2. {workspace_root}/config/tracing.toml
    /// 3. {workspace_root}/.tracing.toml (legacy)
    /// 4. {workspace_root}/tracing.toml (legacy)
    /// 5. ./config/tracing.toml (current directory)
    /// 6. ./.tracing.toml (current directory, legacy)
    /// 7. ./tracing.toml (current directory, legacy)
    /// 8. ~/.config/tracing.toml (user config directory)
    ///
    /// If no config file is found, falls back to from_env()
    pub fn load() -> Self {
        // Check for explicit config file path
        if let Ok(config_path) = env::var("TRACING_CONFIG") {
            let path = PathBuf::from(&config_path);
            // Try as-is first (absolute or relative to cwd)
            if let Ok(config) = Self::from_file(&path) {
                eprintln!("Loaded tracing config from: {}", config_path);
                return config;
            }
            // If not found, try relative to workspace root
            let workspace_path = get_workspace_root().join(&path);
            if let Ok(config) = Self::from_file(&workspace_path) {
                eprintln!(
                    "Loaded tracing config from: {}",
                    workspace_path.display()
                );
                return config;
            }
            eprintln!(
                "Warning: TRACING_CONFIG points to invalid file: {}",
                config_path
            );
        }

        // Get workspace root for tests running from subdirectories
        let workspace_root = get_workspace_root();

        // Build search paths - prioritize workspace config/ directory
        let mut all_paths = vec![
            workspace_root.join("config").join("tracing.toml"),
            workspace_root.join(".tracing.toml"), // legacy
            workspace_root.join("tracing.toml"),  // legacy
            PathBuf::from("config").join("tracing.toml"),
            PathBuf::from(".tracing.toml"),
            PathBuf::from("tracing.toml"),
        ];

        // Add user config directory if available
        if let Some(home) =
            env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))
        {
            let mut user_config = PathBuf::from(home);
            user_config.push(".config");
            user_config.push("tracing.toml");
            all_paths.push(user_config);
        }

        // Try to load from each path
        for path in all_paths {
            if path.exists() {
                match Self::from_file(&path) {
                    Ok(config) => {
                        eprintln!(
                            "Loaded tracing config from: {}",
                            path.display()
                        );
                        return config;
                    },
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to load config from {}: {}",
                            path.display(),
                            e
                        );
                    },
                }
            }
        }

        // No config file found, fall back to environment variables
        eprintln!("No tracing config file found, using environment variables");
        Self::from_env()
    }

    /// Parse from environment variables
    ///
    /// Supports both flat and nested paths:
    /// - TRACING_SPAN_ENTER_SHOW=0
    /// - TRACING_SPAN_ENTER_SHOW_FN_SIGNATURE=0
    /// - TRACING_SPAN_ENTER_SHOW_FIELDS=0
    /// - TRACING_SPAN_CLOSE_SHOW=0
    /// - TRACING_SPAN_CLOSE_SHOW_TIMING=0
    /// - TRACING_PANIC_SHOW=0
    /// - TRACING_PANIC_SHOW_MESSAGE=0
    /// - TRACING_PANIC_SHOW_STDERR=0
    /// - TRACING_ENABLE_INDENTATION=0
    /// - TRACING_SHOW_FILE_LOCATION=0
    /// - TRACING_ENABLE_ANSI=0
    ///
    /// Legacy flat names also supported:
    /// - TRACING_SHOW_FN_SIGNATURE
    /// - TRACING_SHOW_SPAN_FIELDS
    /// - TRACING_SHOW_SPAN_TIMING
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Span enter configuration
        if let Ok(val) = env::var("TRACING_SPAN_ENTER_SHOW") {
            config.span_enter.show =
                val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = env::var("TRACING_SPAN_ENTER_SHOW_FN_SIGNATURE") {
            config.span_enter.show_fn_signature =
                val == "1" || val.eq_ignore_ascii_case("true");
        } else if let Ok(val) = env::var("TRACING_SHOW_FN_SIGNATURE") {
            // Legacy name
            config.span_enter.show_fn_signature =
                val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = env::var("TRACING_SPAN_ENTER_SHOW_FIELDS") {
            config.span_enter.show_fields =
                val == "1" || val.eq_ignore_ascii_case("true");
        } else if let Ok(val) = env::var("TRACING_SHOW_SPAN_FIELDS") {
            // Legacy name
            config.span_enter.show_fields =
                val == "1" || val.eq_ignore_ascii_case("true");
        }

        // Span close configuration
        if let Ok(val) = env::var("TRACING_SPAN_CLOSE_SHOW") {
            config.span_close.show =
                val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = env::var("TRACING_SPAN_CLOSE_SHOW_TIMING") {
            config.span_close.show_timing =
                val == "1" || val.eq_ignore_ascii_case("true");
        } else if let Ok(val) = env::var("TRACING_SHOW_SPAN_TIMING") {
            // Legacy name
            config.span_close.show_timing =
                val == "1" || val.eq_ignore_ascii_case("true");
        }

        // Panic configuration
        if let Ok(val) = env::var("TRACING_PANIC_SHOW") {
            config.panic.show = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = env::var("TRACING_PANIC_SHOW_MESSAGE") {
            config.panic.show_message =
                val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = env::var("TRACING_PANIC_SHOW_STDERR") {
            config.panic.show_stderr =
                val == "1" || val.eq_ignore_ascii_case("true");
        }

        // General formatting configuration
        if let Ok(val) = env::var("TRACING_ENABLE_INDENTATION") {
            config.enable_indentation =
                val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = env::var("TRACING_SHOW_FILE_LOCATION") {
            config.show_file_location =
                val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = env::var("TRACING_ENABLE_ANSI") {
            config.enable_ansi = val == "1" || val.eq_ignore_ascii_case("true");
        }

        config
    }
}

/// Configuration for test tracing
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Directory where log files are stored
    pub log_dir: PathBuf,
    /// Default log level for stdout
    pub stdout_level: Level,
    /// Default log level for file
    pub file_level: Level,
    /// Whether to log to stdout
    pub log_to_stdout: bool,
    /// Whether to log to file
    pub log_to_file: bool,
    /// Custom filter directives for stdout (e.g., "context_search=info,context_trace=error")
    pub stdout_filter_directives: Option<String>,
    /// Custom filter directives for file (e.g., "context_search=trace,context_trace=debug")
    pub file_filter_directives: Option<String>,
    /// Which spans to log
    pub span_events: FmtSpan,
    /// Formatting configuration
    pub format: FormatConfig,
}

impl Default for TracingConfig {
    fn default() -> Self {
        let format = FormatConfig::load();

        // Check environment variable to enable stdout logging
        // Usage: LOG_STDOUT=1 cargo test
        // or:    LOG_STDOUT=true cargo test
        // Config file value takes precedence over env var
        let log_to_stdout = format.log_to_stdout.unwrap_or_else(|| {
            env::var("LOG_STDOUT")
                .map(|v| {
                    v == "1"
                        || v.eq_ignore_ascii_case("true")
                        || v.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
        });

        // Check for log filter in config file, then env var
        let filter_directives = format
            .log_filter
            .clone()
            .or_else(|| env::var("LOG_FILTER").ok());

        Self {
            log_dir: get_target_dir().join("test-logs"),
            stdout_level: Level::DEBUG,
            file_level: Level::TRACE,
            log_to_stdout,
            log_to_file: true,
            stdout_filter_directives: filter_directives.clone(),
            file_filter_directives: filter_directives,
            span_events: FmtSpan::ENTER | FmtSpan::CLOSE,
            format,
        }
    }
}

impl TracingConfig {
    /// Create config with custom log level for both stdout and file
    pub fn with_level(
        mut self,
        level: Level,
    ) -> Self {
        self.stdout_level = level;
        self.file_level = level;
        self
    }

    /// Create config with custom stdout log level
    pub fn with_stdout_level(
        mut self,
        level: Level,
    ) -> Self {
        self.stdout_level = level;
        self
    }

    /// Create config with custom file log level
    pub fn with_file_level(
        mut self,
        level: Level,
    ) -> Self {
        self.file_level = level;
        self
    }

    /// Create config with custom filter directives for both stdout and file
    ///
    /// Example: `"context_search::search=trace,context_trace=debug"`
    pub fn with_filter(
        mut self,
        filter: impl Into<String>,
    ) -> Self {
        let filter_str = filter.into();
        self.stdout_filter_directives = Some(filter_str.clone());
        self.file_filter_directives = Some(filter_str);
        self
    }

    /// Create config with custom filter directives for stdout only
    ///
    /// Example: `"context_search=info,context_trace=error"`
    pub fn with_stdout_filter(
        mut self,
        filter: impl Into<String>,
    ) -> Self {
        self.stdout_filter_directives = Some(filter.into());
        self
    }

    /// Create config with custom filter directives for file only
    ///
    /// Example: `"context_search=trace,context_trace=debug"`
    pub fn with_file_filter(
        mut self,
        filter: impl Into<String>,
    ) -> Self {
        self.file_filter_directives = Some(filter.into());
        self
    }

    /// Enable/disable stdout logging
    pub fn stdout(
        mut self,
        enabled: bool,
    ) -> Self {
        self.log_to_stdout = enabled;
        self
    }

    /// Enable/disable file logging
    pub fn file(
        mut self,
        enabled: bool,
    ) -> Self {
        self.log_to_file = enabled;
        self
    }

    /// Set which span events to log
    pub fn span_events(
        mut self,
        events: FmtSpan,
    ) -> Self {
        self.span_events = events;
        self
    }

    /// Set custom log directory
    pub fn log_dir(
        mut self,
        dir: impl Into<PathBuf>,
    ) -> Self {
        self.log_dir = dir.into();
        self
    }

    /// Set formatting configuration
    pub fn format(
        mut self,
        format: FormatConfig,
    ) -> Self {
        self.format = format;
        self
    }

    /// Enable/disable span enter messages
    pub fn show_span_enter(
        mut self,
        enabled: bool,
    ) -> Self {
        self.format.span_enter.show = enabled;
        self
    }

    /// Enable/disable function signature display
    pub fn show_fn_signature(
        mut self,
        enabled: bool,
    ) -> Self {
        self.format.span_enter.show_fn_signature = enabled;
        self
    }

    /// Enable/disable span fields display
    pub fn show_span_fields(
        mut self,
        enabled: bool,
    ) -> Self {
        self.format.span_enter.show_fields = enabled;
        self
    }

    /// Enable/disable span close messages
    pub fn show_span_close(
        mut self,
        enabled: bool,
    ) -> Self {
        self.format.span_close.show = enabled;
        self
    }

    /// Enable/disable span timing display
    pub fn show_span_timing(
        mut self,
        enabled: bool,
    ) -> Self {
        self.format.span_close.show_timing = enabled;
        self
    }

    /// Enable/disable visual indentation
    pub fn enable_indentation(
        mut self,
        enabled: bool,
    ) -> Self {
        self.format.enable_indentation = enabled;
        self
    }

    /// Enable/disable file location display
    pub fn show_file_location(
        mut self,
        enabled: bool,
    ) -> Self {
        self.format.show_file_location = enabled;
        self
    }

    /// Enable/disable ANSI colors
    pub fn enable_ansi(
        mut self,
        enabled: bool,
    ) -> Self {
        self.format.enable_ansi = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_builder() {
        let config = TracingConfig::default()
            .with_level(Level::TRACE)
            .with_filter("context_search=trace")
            .stdout(false)
            .file(true)
            .log_dir("custom/logs");

        assert_eq!(config.stdout_level, Level::TRACE);
        assert_eq!(config.file_level, Level::TRACE);
        assert_eq!(
            config.stdout_filter_directives,
            Some("context_search=trace".to_string())
        );
        assert_eq!(
            config.file_filter_directives,
            Some("context_search=trace".to_string())
        );
        assert!(!config.log_to_stdout);
        assert!(config.log_to_file);
        assert_eq!(config.log_dir, PathBuf::from("custom/logs"));
    }

    #[test]
    fn test_separate_filters() {
        let config = TracingConfig::default()
            .with_stdout_level(Level::INFO)
            .with_file_level(Level::TRACE)
            .with_stdout_filter("context_search=warn")
            .with_file_filter("context_search=trace,context_trace=debug");

        assert_eq!(config.stdout_level, Level::INFO);
        assert_eq!(config.file_level, Level::TRACE);
        assert_eq!(
            config.stdout_filter_directives,
            Some("context_search=warn".to_string())
        );
        assert_eq!(
            config.file_filter_directives,
            Some("context_search=trace,context_trace=debug".to_string())
        );
    }
}
