use super::{
    WorkflowStep,
    WorkflowSummary,
    WorkflowTemplate,
};

pub(crate) fn available_workflows() -> Vec<WorkflowSummary> {
    vec![
        WorkflowSummary {
            name: "basic",
            title: "Basic End-to-End",
            description: "Create a workspace, add atoms, insert text, search, read, and save. The recommended starting workflow.",
        },
        WorkflowSummary {
            name: "bulk_insert",
            title: "Bulk Text Insertion",
            description: "Efficiently insert many text strings into a workspace using insert_sequences.",
        },
        WorkflowSummary {
            name: "search_and_read",
            title: "Search & Read",
            description: "Search for text in an existing workspace and read back the results.",
        },
        WorkflowSummary {
            name: "inspect",
            title: "Graph Inspection",
            description: "Debug and inspect graph state: statistics, validation, vertex details.",
        },
        WorkflowSummary {
            name: "persistence",
            title: "Save & Reopen",
            description: "Save a workspace to disk, close it, and reopen it later.",
        },
        WorkflowSummary {
            name: "trace_and_debug",
            title: "Trace & Debug",
            description: "Execute a command with tracing enabled, then analyze the resulting trace log.",
        },
    ]
}

pub(crate) fn build_workflow(
    name: &str,
    ws: &str,
) -> Option<WorkflowTemplate> {
    let ws_val = serde_json::Value::String(ws.to_string());

    match name {
        "basic" => Some(WorkflowTemplate {
            name: "basic",
            title: "Basic End-to-End Workflow",
            description: "The standard workflow: create a workspace, populate it with atoms and text, search, read results, and save.",
            steps: vec![
                WorkflowStep {
                    step: 1,
                    description: "Create a new workspace",
                    command: serde_json::json!({
                        "command": "create_workspace",
                        "name": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 2,
                    description: "Add atoms for all unique characters your text will use",
                    command: serde_json::json!({
                        "command": "add_atoms",
                        "workspace": ws_val,
                        "chars": ["h", "e", "l", "o", " ", "w", "r", "d"],
                    }),
                },
                WorkflowStep {
                    step: 3,
                    description: "Insert text sequences — this builds the hypergraph hierarchy",
                    command: serde_json::json!({
                        "command": "insert_sequence",
                        "workspace": ws_val,
                        "text": "hello world",
                    }),
                },
                WorkflowStep {
                    step: 4,
                    description: "Search for a substring to find its token",
                    command: serde_json::json!({
                        "command": "search_sequence",
                        "workspace": ws_val,
                        "text": "hello",
                    }),
                },
                WorkflowStep {
                    step: 5,
                    description: "Read back text from a token index (use the index from search/insert results)",
                    command: serde_json::json!({
                        "command": "read_as_text",
                        "workspace": ws_val,
                        "index": 5,
                    }),
                },
                WorkflowStep {
                    step: 6,
                    description: "Check graph statistics",
                    command: serde_json::json!({
                        "command": "get_statistics",
                        "workspace": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 7,
                    description: "Save the workspace to disk before ending",
                    command: serde_json::json!({
                        "command": "save_workspace",
                        "name": ws_val,
                    }),
                },
            ],
            tips: vec![
                "Always add_atoms BEFORE insert_sequence — insertion requires atoms to exist.",
                "The index returned by insert_sequence can be used with read_as_text.",
                "insert_sequence returns already_existed=true if the text was already in the graph.",
                "Always save_workspace before ending your session.",
            ],
        }),

        "bulk_insert" => Some(WorkflowTemplate {
            name: "bulk_insert",
            title: "Bulk Text Insertion",
            description: "Insert many text strings efficiently. Use insert_sequences for batch operations.",
            steps: vec![
                WorkflowStep {
                    step: 1,
                    description: "Create or open a workspace",
                    command: serde_json::json!({
                        "command": "create_workspace",
                        "name": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 2,
                    description: "Add atoms for ALL unique characters across all texts",
                    command: serde_json::json!({
                        "command": "add_atoms",
                        "workspace": ws_val,
                        "chars": ["a","b","c","d","e","f","g","h","i","j","k","l","m","n","o","p","q","r","s","t","u","v","w","x","y","z"," "],
                    }),
                },
                WorkflowStep {
                    step: 3,
                    description: "Batch-insert multiple sequences at once",
                    command: serde_json::json!({
                        "command": "insert_sequences",
                        "workspace": ws_val,
                        "texts": ["hello world", "hello there", "world peace"],
                    }),
                },
                WorkflowStep {
                    step: 4,
                    description: "Verify with statistics",
                    command: serde_json::json!({
                        "command": "get_statistics",
                        "workspace": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 5,
                    description: "Save",
                    command: serde_json::json!({
                        "command": "save_workspace",
                        "name": ws_val,
                    }),
                },
            ],
            tips: vec![
                "insert_sequences is more efficient than calling insert_sequence in a loop.",
                "Collect ALL unique characters across all texts before calling add_atoms.",
                "The texts parameter is a set — duplicates are automatically ignored.",
            ],
        }),

        "search_and_read" => Some(WorkflowTemplate {
            name: "search_and_read",
            title: "Search & Read Workflow",
            description: "Search for text patterns in an existing workspace, then read the results.",
            steps: vec![
                WorkflowStep {
                    step: 1,
                    description: "Open an existing workspace (skip if already open)",
                    command: serde_json::json!({
                        "command": "open_workspace",
                        "name": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 2,
                    description: "Search for text — check if complete=true in the result",
                    command: serde_json::json!({
                        "command": "search_sequence",
                        "workspace": ws_val,
                        "text": "hello",
                    }),
                },
                WorkflowStep {
                    step: 3,
                    description: "If search found a token, read it as text to confirm",
                    command: serde_json::json!({
                        "command": "read_as_text",
                        "workspace": ws_val,
                        "index": "<use token.index from search result>",
                    }),
                },
                WorkflowStep {
                    step: 4,
                    description: "Read the full tree structure for deeper inspection",
                    command: serde_json::json!({
                        "command": "read_pattern",
                        "workspace": ws_val,
                        "index": "<use token.index from search result>",
                    }),
                },
            ],
            tips: vec![
                "SearchResult.complete=true means the full text exists as one token.",
                "SearchResult.partial contains prefix/postfix matches when complete=false.",
                "SearchResult.query_exhausted=true means all query characters were consumed (text may be a prefix of a larger token).",
                "The token.index in the SearchResult is what you pass to read_as_text and read_pattern.",
            ],
        }),

        "inspect" => Some(WorkflowTemplate {
            name: "inspect",
            title: "Graph Inspection Workflow",
            description: "Debug and understand the current state of a workspace's graph.",
            steps: vec![
                WorkflowStep {
                    step: 1,
                    description: "Check statistics first (lightweight)",
                    command: serde_json::json!({
                        "command": "get_statistics",
                        "workspace": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 2,
                    description: "Validate graph integrity",
                    command: serde_json::json!({
                        "command": "validate_graph",
                        "workspace": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 3,
                    description: "Show the full graph (small graphs only)",
                    command: serde_json::json!({
                        "command": "show_graph",
                        "workspace": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 4,
                    description: "Inspect a specific vertex",
                    command: serde_json::json!({
                        "command": "show_vertex",
                        "workspace": ws_val,
                        "index": 0,
                    }),
                },
                WorkflowStep {
                    step: 5,
                    description: "Get detailed vertex info",
                    command: serde_json::json!({
                        "command": "get_vertex",
                        "workspace": ws_val,
                        "index": 0,
                    }),
                },
            ],
            tips: vec![
                "Always start with get_statistics to know the graph size.",
                "show_graph can produce very large output — avoid on graphs with >100 vertices.",
                "validate_graph checks structural integrity; an empty issues array means healthy.",
                "show_vertex is more focused than show_graph — use it for specific vertices.",
            ],
        }),

        "persistence" => Some(WorkflowTemplate {
            name: "persistence",
            title: "Save & Reopen Workflow",
            description: "Save workspace state to disk, close it, and reopen later.",
            steps: vec![
                WorkflowStep {
                    step: 1,
                    description: "Save the workspace to disk",
                    command: serde_json::json!({
                        "command": "save_workspace",
                        "name": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 2,
                    description: "Close the workspace (frees memory)",
                    command: serde_json::json!({
                        "command": "close_workspace",
                        "name": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 3,
                    description: "Later: reopen the workspace from disk",
                    command: serde_json::json!({
                        "command": "open_workspace",
                        "name": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 4,
                    description: "Verify it loaded correctly",
                    command: serde_json::json!({
                        "command": "get_statistics",
                        "workspace": ws_val,
                    }),
                },
            ],
            tips: vec![
                "ALWAYS save before close — close does NOT auto-save!",
                "Workspaces are saved to <base_dir>/.context-engine/<name>/.",
                "list_workspaces only shows in-memory workspaces; saved-but-closed ones won't appear.",
                "After open_workspace, the graph is fully in memory and ready for operations.",
            ],
        }),

        "trace_and_debug" => Some(WorkflowTemplate {
            name: "trace_and_debug",
            title: "Trace & Debug Workflow",
            description: "Execute a command with tracing enabled, then analyze the trace log to understand engine internals.",
            steps: vec![
                WorkflowStep {
                    step: 1,
                    description: "Execute a command with tracing enabled (note: set trace:true at the top level of the execute input)",
                    command: serde_json::json!({
                        "command": "insert_sequence",
                        "workspace": ws_val,
                        "text": "hello world",
                        "trace": true,
                    }),
                },
                WorkflowStep {
                    step: 2,
                    description: "List log files to find the trace that was just created",
                    command: serde_json::json!({
                        "command": "list_logs",
                        "workspace": ws_val,
                    }),
                },
                WorkflowStep {
                    step: 3,
                    description: "Analyze the log for a quick overview (replace filename with the actual one from step 2)",
                    command: serde_json::json!({
                        "command": "analyze_log",
                        "workspace": ws_val,
                        "filename": "<use filename from list_logs>",
                    }),
                },
                WorkflowStep {
                    step: 4,
                    description: "Query for specific event types like graph operations",
                    command: serde_json::json!({
                        "command": "query_log",
                        "workspace": ws_val,
                        "filename": "<use filename from list_logs>",
                        "query": "select(.fields.graph_op != null)",
                    }),
                },
                WorkflowStep {
                    step: 5,
                    description: "Search for errors or warnings",
                    command: serde_json::json!({
                        "command": "query_log",
                        "workspace": ws_val,
                        "filename": "<use filename from list_logs>",
                        "query": "select(.level == \"WARN\" or .level == \"ERROR\")",
                    }),
                },
            ],
            tips: vec![
                "The trace flag only affects the command it's attached to.",
                "Log files persist across sessions in the workspace directory.",
                "JQ queries use the jaq engine — most standard jq syntax works.",
                "Use analyze_log first for a quick overview before deep-querying.",
                "graph_op fields contain rich structured data about algorithm steps.",
            ],
        }),

        _ => None,
    }
}