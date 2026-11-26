//! A stderr progress line for large reads, fed by `ReadProgress` events.

use std::{
    io::Write,
    sync::Arc,
};

use context_api::{
    Event,
    EventSink,
};

/// Inputs above this size (bytes, all inputs together) show progress.
pub const PROGRESS_THRESHOLD: usize = 1 << 20;

/// An event sink that redraws one progress line on stderr.
pub fn sink() -> EventSink {
    Arc::new(move |event: &Event| {
        if let Event::ReadProgress {
            input,
            chars,
            total,
            ..
        } = event
        {
            let percent = if *total == 0 {
                100
            } else {
                chars * 100 / total
            };
            let mut err = std::io::stderr().lock();
            let _ = write!(
                err,
                "\r\x1b[Kreading input {}: {chars}/{total} chars ({percent}%)",
                input + 1
            );
            let _ = err.flush();
        }
    })
}

/// Clear the progress line.
pub fn finish() {
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\r\x1b[K");
    let _ = err.flush();
}
