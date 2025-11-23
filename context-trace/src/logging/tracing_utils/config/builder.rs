//! Builder methods and configuration helpers

use super::loader::TracingConfig;
use super::types::FormatConfig;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;

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
