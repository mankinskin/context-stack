//! Scoped tracing capture for per-command log files.
//!
//! Provides `build_capture_dispatch()` which creates a `tracing::Dispatch`
//! that writes pretty-printed JSON to a file — the same format as
//! `TestTracing`, compatible with `LogParser`.

use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::AtomicUsize,
    },
};

use tracing::Dispatch;
use tracing_subscriber::{
    EnvFilter,
    Layer,
    layer::SubscriberExt,
};

use super::{
    debug_to_json::{
        self,
        SignatureStore,
    },
    special_fields::SpecialFieldExtractor,
    writers::{
        FlushingWriter,
        PrettyJsonWriter,
    },
};

/// Handle returned from [`build_capture_dispatch`].
///
/// Holds the `Dispatch` and shared metadata. Use with
/// `tracing::dispatcher::with_default(&capture.dispatch, || { ... })`.
pub struct CaptureDispatch {
    /// The dispatch to use with `tracing::dispatcher::with_default`.
    pub dispatch: Dispatch,
    /// Shared event counter — read after the dispatch scope ends.
    pub event_count: Arc<AtomicUsize>,
    /// Signature store for collected fn_sig entries.
    pub signatures: SignatureStore,
}

/// Build a [`tracing::Dispatch`] that captures events to a JSON log file.
///
/// The returned dispatch writes pretty-printed JSON to `log_file_path`
/// in the same format as `TestTracing`, compatible with `LogParser`.
///
/// # Arguments
///
/// * `log_file_path` — Where to write the JSON log.
/// * `level_filter` — A `tracing` filter directive string, e.g. `"TRACE"` or
///   `"context_search=DEBUG,context_insert=TRACE"`.
///
/// # Usage
///
/// ```ignore
/// let capture = build_capture_dispatch(&log_path, "TRACE")?;
/// let result = tracing::dispatcher::with_default(&capture.dispatch, || {
///     do_something_that_emits_tracing_events()
/// });
/// let count = capture.event_count.load(std::sync::atomic::Ordering::Relaxed);
/// ```
pub fn build_capture_dispatch(
    log_file_path: &Path,
    level_filter: &str,
) -> Result<CaptureDispatch, Box<dyn std::error::Error + Send + Sync>> {
    // Ensure parent directory exists
    if let Some(parent) = log_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = fs::File::create(log_file_path)?;
    let flushing = FlushingWriter::new(file);
    let signatures = debug_to_json::new_signature_store();
    let writer = PrettyJsonWriter::new(flushing, signatures.clone());

    let filter = EnvFilter::try_new(level_filter)
        .unwrap_or_else(|_| EnvFilter::new("TRACE"));

    let event_count = Arc::new(AtomicUsize::new(0));
    let counter = event_count.clone();

    // Build a counting layer that increments on each event
    let counting_layer = CountingLayer { counter };

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(move || writer.clone())
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::ENTER
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_ansi(false)
        .json()
        .with_filter(filter);

    let registry = tracing_subscriber::registry();
    let dispatch = Dispatch::new(
        registry
            .with(SpecialFieldExtractor)
            .with(counting_layer)
            .with(file_layer),
    );

    Ok(CaptureDispatch {
        dispatch,
        event_count,
        signatures,
    })
}

/// A simple layer that counts events.
struct CountingLayer {
    counter: Arc<AtomicUsize>,
}

impl<S> Layer<S> for CountingLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        _event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}
