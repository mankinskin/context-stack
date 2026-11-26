//! Events pushed to subscribers (server WebSocket, shell, viewer).

use serde::{
    Deserialize,
    Serialize,
};

use crate::ts_export;

ts_export! {
    /// Event topics a client can subscribe to.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Topic {
        GraphChanged,
        ReadProgress,
        Saved,
    }

    /// An event about a graph.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(tag = "event", rename_all = "snake_case")]
    pub enum Event {
        /// Progress of a running read; throttled by the emitter.
        ReadProgress {
            graph: String,
            /// Id of the request that runs the read.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            request: Option<u64>,
            /// Index of the input within the request.
            input: usize,
            /// Chars of this input consumed so far.
            chars: usize,
            /// Chars of this input.
            total: usize,
        },
        /// A mutating command changed the graph.
        GraphChanged {
            graph: String,
            generation: u64,
            vertex_count: usize,
            new_inputs: Vec<u32>,
        },
        Saved { graph: String, generation: u64 },
        /// The subscriber fell behind; `skipped` events were dropped.
        Lagged { skipped: u64 },
    }
}

impl Event {
    pub fn topic(&self) -> Option<Topic> {
        match self {
            Event::ReadProgress { .. } => Some(Topic::ReadProgress),
            Event::GraphChanged { .. } => Some(Topic::GraphChanged),
            Event::Saved { .. } => Some(Topic::Saved),
            Event::Lagged { .. } => None,
        }
    }

    pub fn graph(&self) -> Option<&str> {
        match self {
            Event::ReadProgress { graph, .. }
            | Event::GraphChanged { graph, .. }
            | Event::Saved { graph, .. } => Some(graph),
            Event::Lagged { .. } => None,
        }
    }
}

/// Receiver of session events; must not block.
pub type EventSink = std::sync::Arc<dyn Fn(&Event) + Send + Sync>;
