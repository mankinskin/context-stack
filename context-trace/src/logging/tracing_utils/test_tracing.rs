//! Main TestTracing API and initialization

use std::{
    env,
    fs,
    io::Write,
    path::{
        Path,
        PathBuf,
    },
    sync::Once,
};
use tracing::Dispatch;
use tracing_subscriber::{
    EnvFilter,
    Layer,
    layer::SubscriberExt,
};

use super::{
    config::TracingConfig,
    formatter::CompactFieldsFormatter,
    panic::install_panic_hook,
    timer::CompactTimer,
};

static GLOBAL_INIT: Once = Once::new();

/// A file wrapper that flushes after every write to ensure logs are visible on panic.
///
/// This is necessary because when a test panics, buffered data may not be flushed
/// to disk, resulting in truncated log files.
#[derive(Clone)]
struct FlushingWriter {
    file: std::sync::Arc<std::sync::Mutex<fs::File>>,
}

impl FlushingWriter {
    fn new(file: fs::File) -> Self {
        Self {
            file: std::sync::Arc::new(std::sync::Mutex::new(file)),
        }
    }
}

impl Write for FlushingWriter {
    fn write(
        &mut self,
        buf: &[u8],
    ) -> std::io::Result<usize> {
        // Use lock().ok() to handle poisoned mutex during panic gracefully
        // If we can't get the lock (e.g., during unwind), skip the write
        let Some(mut file) = self.file.lock().ok().or_else(|| {
            // Mutex is poisoned, try to recover it
            self.file.clear_poison();
            self.file.lock().ok()
        }) else {
            return Err(std::io::Error::other("Failed to acquire file lock"));
        };
        let result = file.write(buf)?;
        file.flush()?;
        Ok(result)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(mut file) = self.file.lock().ok().or_else(|| {
            self.file.clear_poison();
            self.file.lock().ok()
        }) {
            file.flush()
        } else {
            Ok(())
        }
    }
}

/// A writer that pretty-prints JSON output with indentation
/// 
/// Wraps another writer and buffers JSON objects. When a complete JSON
/// object is detected, it's parsed and re-serialized with indentation.
#[derive(Clone)]
struct PrettyJsonWriter<W> {
    inner: W,
    buffer: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl<W: Clone> PrettyJsonWriter<W> {
    fn new(writer: W) -> Self {
        Self {
            inner: writer,
            buffer: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl<W: Write + Clone> Write for PrettyJsonWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut buffer = self.buffer.lock().map_err(|_| {
            std::io::Error::other("Failed to acquire buffer lock")
        })?;
        
        // Add incoming data to buffer
        buffer.extend_from_slice(buf);
        
        // Check if we have a complete JSON object (ends with newline)
        if buffer.ends_with(b"\n") {
            // Try to parse and pretty-print the JSON
            if let Ok(json_str) = std::str::from_utf8(&buffer) {
                let trimmed = json_str.trim();
                if !trimmed.is_empty() {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        // Write pretty-printed JSON
                        let pretty = serde_json::to_string_pretty(&value)
                            .unwrap_or_else(|_| trimmed.to_string());
                        let mut inner = self.inner.clone();
                        inner.write_all(pretty.as_bytes())?;
                        inner.write_all(b"\n\n")?; // Double newline between entries
                        inner.flush()?;
                        buffer.clear();
                        return Ok(buf.len());
                    }
                }
            }
            
            // Fallback: write raw data if JSON parsing fails
            let mut inner = self.inner.clone();
            inner.write_all(&buffer)?;
            inner.flush()?;
            buffer.clear();
        }
        
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.clone().flush()
    }
}

/// Trait for types that can provide access to a Hypergraph for test graph registration
#[cfg(any(test, feature = "test-api"))]
pub trait AsGraphRef<G: crate::graph::kind::GraphKind> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display;
}

#[cfg(any(test, feature = "test-api"))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G> for &crate::Hypergraph<G> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        crate::graph::test_graph::register_test_graph(self);
    }
}

#[cfg(any(test, feature = "test-api"))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G>
    for &crate::HypergraphRef<G>
{
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        // Use the new register_test_graph_ref to avoid cloning
        crate::graph::test_graph::register_test_graph_ref(self);
    }
}

#[cfg(any(test, feature = "test-api"))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G> for crate::Hypergraph<G> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        crate::graph::test_graph::register_test_graph(&self);
    }
}

#[cfg(any(test, feature = "test-api"))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G>
    for crate::HypergraphRef<G>
{
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        // Use the new register_test_graph_ref to avoid cloning
        crate::graph::test_graph::register_test_graph_ref(&self);
    }
}

/// Guard that handles test logging lifecycle
///
/// Automatically cleans up log files when the test succeeds (guard is dropped without panic).
/// Also handles test graph registration and cleanup.
/// The guard holds a tracing dispatcher that's active for the lifetime of the test.
pub struct TestTracing {
    log_file_path: Option<PathBuf>,
    keep_success_logs: bool,
    clear_test_graph_on_drop: bool,
    _dispatcher: Dispatch,
    _guard: tracing::dispatcher::DefaultGuard,
}

impl TestTracing {
    /// Initialize tracing for a test
    ///
    /// # Example
    /// ```no_run
    /// use context_trace::logging::tracing_utils::TestTracing;
    ///
    /// #[test]
    /// fn my_test() {
    ///     let _tracing = TestTracing::init("my_test");
    ///     // Test code with automatic tracing
    ///     // Log file will be deleted if test passes
    /// }
    /// ```
    pub fn init(test_name: &str) -> Self {
        Self::init_with_config(test_name, TracingConfig::default())
    }

    /// Initialize tracing with custom configuration
    pub fn init_with_config(
        test_name: &str,
        config: TracingConfig,
    ) -> Self {
        Self::init_internal(test_name, config, false)
    }

    /// Initialize tracing and register a test graph
    ///
    /// # Example
    /// ```no_run
    /// use context_trace::{Hypergraph, logging::tracing_utils::TestTracing};
    ///
    /// #[test]
    /// fn my_test() {
    ///     let graph = Hypergraph::default();
    ///     let _tracing = TestTracing::init_with_graph("my_test", &graph);
    ///     // Test code - tokens will show string representations
    ///     // Graph and log file will be cleaned up if test passes
    /// }
    /// ```
    #[cfg(any(test, feature = "test-api"))]
    pub fn init_with_graph<G>(
        test_name: &str,
        graph: impl AsGraphRef<G>,
    ) -> Self
    where
        G: crate::graph::kind::GraphKind + Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        graph.register_test_graph();
        Self::init_internal(test_name, TracingConfig::default(), true)
    }

    /// Initialize tracing with custom configuration and register a test graph
    #[cfg(any(test, feature = "test-api"))]
    pub fn init_with_config_and_graph<G>(
        test_name: &str,
        config: TracingConfig,
        graph: impl AsGraphRef<G>,
    ) -> Self
    where
        G: crate::graph::kind::GraphKind + Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        graph.register_test_graph();
        Self::init_internal(test_name, config, true)
    }

    fn init_internal(
        test_name: &str,
        config: TracingConfig,
        clear_test_graph_on_drop: bool,
    ) -> Self {
        // Initialize global tracing only once
        GLOBAL_INIT.call_once(|| {
            // This is a placeholder - actual subscriber will be set per-test
        });

        // Install panic hook to log panics before spans close
        install_panic_hook(config.format.panic.clone());

        // Create log directory
        if config.log_to_file {
            fs::create_dir_all(&config.log_dir).ok();
        }

        let log_file_path = if config.log_to_file {
            Some(config.log_dir.join(format!("{}.log", test_name)))
        } else {
            None
        };

        // Build separate filters for stdout and file
        let stdout_filter = if let Some(directives) =
            &config.stdout_filter_directives
        {
            EnvFilter::try_new(directives).unwrap_or_else(|_| {
                EnvFilter::new(config.stdout_level.as_str())
            })
        } else {
            // Check for LOG_FILTER first (preferred), then RUST_LOG, otherwise use stdout level
            env::var("LOG_FILTER")
                .ok()
                .and_then(|filter| EnvFilter::try_new(&filter).ok())
                .or_else(|| EnvFilter::try_from_default_env().ok())
                .unwrap_or_else(|| EnvFilter::new(config.stdout_level.as_str()))
        };

        let file_filter = if let Some(directives) =
            &config.file_filter_directives
        {
            EnvFilter::try_new(directives)
                .unwrap_or_else(|_| EnvFilter::new(config.file_level.as_str()))
        } else {
            // For file output, also check LOG_FILTER
            env::var("LOG_FILTER")
                .ok()
                .and_then(|filter| EnvFilter::try_new(&filter).ok())
                .unwrap_or_else(|| EnvFilter::new(config.file_level.as_str()))
        };

        // Create the subscriber without a global filter
        let registry = tracing_subscriber::registry();

        // Extract config values to avoid partial move issues
        let span_events = config.span_events;
        let log_to_stdout = config.log_to_stdout;
        let format_config = config.format.clone();

        // Build layers based on configuration
        // Timestamp display is controlled by the formatter's show_timestamp config,
        // so we always use CompactTimer and let the formatter decide whether to call format_time.
        // For file output, we use JSON format for easy parsing by the log viewer
        // Create dispatcher based on configuration
        let dispatcher = match (log_to_stdout, log_file_path.as_ref()) {
            (true, Some(path)) => {
                // Both stdout and file
                let file =
                    fs::File::create(path).expect("Failed to create log file");
                let flushing_writer = FlushingWriter::new(file);
                let pretty_writer = PrettyJsonWriter::new(flushing_writer);

                let stdout_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stdout)
                    .with_span_events(span_events.clone())
                    .with_target(false)
                    .with_file(false)
                    .with_line_number(false)
                    .with_level(false)
                    .with_ansi(format_config.enable_ansi)
                    .with_timer(CompactTimer::new())
                    .event_format(CompactFieldsFormatter::new(
                        format_config.clone(),
                    ))
                    .fmt_fields(super::SpanFieldFormatter)
                    .with_filter(stdout_filter);

                // File layer uses pretty-printed JSON format for human readability
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(move || pretty_writer.clone())
                    .with_span_events(span_events)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_level(true)
                    .with_ansi(false)
                    .json()
                    .with_filter(file_filter);

                Dispatch::new(registry.with(stdout_layer).with(file_layer))
            },
            (true, None) => {
                // Only stdout
                let stdout_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stdout)
                    .with_span_events(span_events)
                    .with_target(false)
                    .with_file(false)
                    .with_line_number(false)
                    .with_level(false)
                    .with_ansi(format_config.enable_ansi)
                    .with_timer(CompactTimer::new())
                    .event_format(CompactFieldsFormatter::new(format_config))
                    .fmt_fields(super::SpanFieldFormatter)
                    .with_filter(stdout_filter);

                Dispatch::new(registry.with(stdout_layer))
            },
            (false, Some(path)) => {
                // Only file - use pretty-printed JSON format for human readability
                let file =
                    fs::File::create(path).expect("Failed to create log file");
                let flushing_writer = FlushingWriter::new(file);
                let pretty_writer = PrettyJsonWriter::new(flushing_writer);

                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(move || pretty_writer.clone())
                    .with_span_events(span_events)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_level(true)
                    .with_ansi(false)
                    .json()
                    .with_filter(file_filter);

                Dispatch::new(registry.with(file_layer))
            },
            (false, None) => {
                // No output - minimal subscriber
                Dispatch::new(registry)
            },
        };

        // Set as the default dispatcher for this test's scope
        let guard = tracing::dispatcher::set_default(&dispatcher);

        tracing::info!(
            test_name = %test_name,
            log_file = ?log_file_path,
            "Test tracing initialized"
        );

        Self {
            log_file_path,
            keep_success_logs: config.keep_success_logs,
            clear_test_graph_on_drop,
            _dispatcher: dispatcher,
            _guard: guard,
        }
    }

    /// Get the path to the log file for this test
    pub fn log_file(&self) -> Option<&Path> {
        self.log_file_path.as_deref()
    }

    /// Explicitly keep the log file (don't delete on drop)
    ///
    /// Useful if you want to preserve logs even for passing tests
    pub fn keep_log(mut self) -> Self {
        self.keep_success_logs = true;
        self
    }

    /// Explicitly clear the test graph on drop
    ///
    /// Useful if you manually registered a test graph but didn't use init_with_graph
    pub fn clear_test_graph(mut self) -> Self {
        self.clear_test_graph_on_drop = true;
        self
    }
}

impl Drop for TestTracing {
    fn drop(&mut self) {
        // Check if we're unwinding (test panicked/failed)
        let is_panicking = std::thread::panicking();

        if !is_panicking && !self.keep_success_logs {
            // Test passed and keep_success_logs disabled - clean up log file
            if let Some(ref path) = self.log_file_path {
                tracing::info!(
                    log_file = %path.display(),
                    "Test passed, removing log file"
                );
                fs::remove_file(path).ok();
            }
        } else {
            // Test failed or keep_success_logs enabled - keep log file
            if let Some(ref path) = self.log_file_path {
                if is_panicking {
                    eprintln!(
                        "\n❌ Test failed! Log file preserved at: {}",
                        path.display()
                    );
                } else if self.keep_success_logs {
                    eprintln!(
                        "\n📝 Test passed! Log file kept at: {}",
                        path.display()
                    );
                }
            }
        }

        // Clear test graph if requested
        #[cfg(any(test, feature = "test-api"))]
        if self.clear_test_graph_on_drop {
            crate::graph::test_graph::clear_test_graph();
        }
    }
}
