//! Configuration file loading and environment variable parsing

use super::types::FormatConfig;
use crate::logging::tracing_utils::path::{
    get_target_dir,
    get_workspace_root,
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

impl FormatConfig {
    /// Load configuration from a TOML file
    ///
    /// Note: In TOML, top-level keys MUST appear before any [section] headers.
    /// Keys after a [section] header are part of that section, not top-level.
    ///
    /// Example file format:
    /// ```toml
    /// # Top-level options (must be before any [section])
    /// enable_indentation = true
    /// show_file_location = true
    /// enable_ansi = true
    /// show_timestamp = true
    ///
    /// # Optional: fallback values when environment variables are not set
    /// log_to_stdout = true
    /// log_filter = "debug"           # Applies to both stdout and file (fallback)
    /// stdout_log_filter = "info"     # Specific filter for stdout output
    /// file_log_filter = "trace"      # Specific filter for file output
    ///
    /// [span_enter]
    /// show = true
    /// show_fn_signature = true
    /// show_fields = true
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
    /// - TRACING_PANIC_SHOW_DEFAULT_HOOK=0
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
        if let Ok(val) = env::var("TRACING_PANIC_SHOW_DEFAULT_HOOK") {
            config.panic.show_default_hook =
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
    /// Keep log files even when tests pass
    pub keep_success_logs: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        let format = FormatConfig::load();

        // Check environment variable to enable stdout logging
        // Usage: LOG_STDOUT=1 cargo test
        // or:    LOG_STDOUT=true cargo test
        // Env var takes precedence over config file value
        let log_to_stdout = env::var("LOG_STDOUT")
            .ok()
            .map(|v| {
                v == "1"
                    || v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("yes")
            })
            .or(format.log_to_stdout)
            .unwrap_or(false);

        // Check for log filter: env vars always take precedence over config file values
        // Priority for stdout: LOG_STDOUT_FILTER > LOG_FILTER > config.stdout_log_filter > config.log_filter
        let stdout_filter_directives = env::var("LOG_STDOUT_FILTER")
            .ok()
            .or_else(|| env::var("LOG_FILTER").ok())
            .or_else(|| format.stdout_log_filter.clone())
            .or_else(|| format.log_filter.clone());

        // Priority for file: LOG_FILE_FILTER > LOG_FILTER > config.file_log_filter > config.log_filter
        let file_filter_directives = env::var("LOG_FILE_FILTER")
            .ok()
            .or_else(|| env::var("LOG_FILTER").ok())
            .or_else(|| format.file_log_filter.clone())
            .or_else(|| format.log_filter.clone());

        // Check for keep logs: env var takes precedence over config file
        let keep_success_logs = env::var("KEEP_SUCCESS_LOGS")
            .ok()
            .map(|v| {
                v == "1"
                    || v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("yes")
            })
            .or(format.keep_success_logs)
            .unwrap_or(false);

        Self {
            log_dir: get_target_dir().join("test-logs"),
            stdout_level: Level::DEBUG,
            file_level: Level::TRACE,
            log_to_stdout,
            log_to_file: true,
            stdout_filter_directives,
            file_filter_directives,
            span_events: FmtSpan::ENTER | FmtSpan::CLOSE,
            format,
            keep_success_logs,
        }
    }
}
