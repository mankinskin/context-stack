//! Command execution backends. `LocalBackend` runs requests in process;
//! stage 4 adds a remote backend behind the same trait.

use std::path::PathBuf;

pub use context_api::Backend;
use context_api::{
    EventSink,
    Request,
    Response,
    SaveMode,
    Session,
    SessionConfig,
};

/// An in-process [`Session`] for one-shot commands: every command opens the
/// graphs it uses in the mode it needs (shared for queries, exclusive for
/// mutations), saves mutations immediately and releases the locks.
pub struct LocalBackend {
    session: Session,
}

impl LocalBackend {
    pub fn new(
        store: PathBuf,
        events: Option<EventSink>,
    ) -> Self {
        let mut config = SessionConfig::new(store);
        config.keep_open = false;
        config.save = SaveMode::OnWrite;
        // Files are read by the CLI and sent as text.
        config.allow_fs = None;
        let mut session = Session::new(config);
        if let Some(sink) = events {
            session.set_event_sink(sink);
        }
        Self { session }
    }
}

impl Backend for LocalBackend {
    fn execute(
        &mut self,
        request: Request,
    ) -> Response {
        self.session.execute(request)
    }
}
