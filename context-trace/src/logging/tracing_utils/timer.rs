//! Time formatting utilities

use std::time::SystemTime as StdSystemTime;
use tracing_subscriber::fmt::{
    format,
    time::FormatTime,
};

/// Compact timer that shows milliseconds since start
pub(super) struct CompactTimer {
    start: StdSystemTime,
}

impl CompactTimer {
    pub(super) fn new() -> Self {
        Self {
            start: StdSystemTime::now(),
        }
    }
}

impl FormatTime for CompactTimer {
    fn format_time(
        &self,
        w: &mut format::Writer<'_>,
    ) -> std::fmt::Result {
        let elapsed = StdSystemTime::now()
            .duration_since(self.start)
            .unwrap_or_default();

        let millis = elapsed.as_millis();

        // Format as seconds.milliseconds (e.g., "1.234s" or "0.056s")
        if millis < 1000 {
            write!(w, "{:3}ms", millis)
        } else if millis < 60_000 {
            write!(w, "{:5.2}s", millis as f64 / 1000.0)
        } else {
            let minutes = millis / 60_000;
            let remaining_ms = millis % 60_000;
            write!(w, "{}m{:05.2}s", minutes, remaining_ms as f64 / 1000.0)
        }
    }
}
