mod diagnostics;
mod text;
mod workspace;

use super::{
    CategoryInfo,
    CommandHelp,
};

pub(crate) fn all_commands() -> Vec<CommandHelp> {
    let mut commands = workspace::workspace_commands();
    commands.extend(text::text_commands());
    commands.extend(diagnostics::diagnostics_commands());
    commands
}

pub(crate) fn all_categories() -> Vec<CategoryInfo> {
    vec![
        CategoryInfo {
            name: "workspace",
            display: "Workspace Lifecycle",
            description: "Create, open, save, close, and delete workspaces.",
        },
        CategoryInfo {
            name: "atoms",
            display: "Atoms",
            description: "Manage character atoms — the leaf nodes of the hypergraph.",
        },
        CategoryInfo {
            name: "patterns",
            display: "Patterns & Vertices",
            description: "Create compound patterns and inspect vertices.",
        },
        CategoryInfo {
            name: "search",
            display: "Search",
            description: "Search for text or token patterns in the graph.",
        },
        CategoryInfo {
            name: "insert",
            display: "Insert",
            description: "Insert text or token sequences, building the hypergraph hierarchy.",
        },
        CategoryInfo {
            name: "read",
            display: "Read",
            description: "Read back text and tree structures from the graph.",
        },
        CategoryInfo {
            name: "debug",
            display: "Debug & Inspection",
            description: "Snapshots, statistics, validation, and visual display.",
        },
        CategoryInfo {
            name: "logs",
            display: "Trace Logs",
            description: "Access and query per-command tracing logs. Enable tracing via trace:true in execute.",
        },
    ]
}