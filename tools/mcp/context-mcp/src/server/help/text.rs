use crate::server::{
    CommandHelp,
    ParamHelp,
};

pub(super) fn text_commands() -> Vec<CommandHelp> {
    vec![
        // -- Search --
        CommandHelp {
            name: "search_pattern",
            category: "search",
            summary: "Search for a pattern using token references (index or label).",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "query",
                    param_type: "array of TokenRef",
                    required: true,
                    description: r#"Sequence of token references. Each is {"index": N} or {"label": "text"}."#,
                },
            ],
            example: r#"{"command": "search_pattern", "workspace": "demo", "query": [{"label": "ab"}, {"label": "c"}]}"#,
            result_type: "SearchResult { complete, token, query_exhausted, partial }",
            notes: "Advanced — use search_sequence for text-based queries instead.",
        },
        CommandHelp {
            name: "search_sequence",
            category: "search",
            summary: "Search for a text string in the graph.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "text",
                    param_type: "string",
                    required: true,
                    description: "Text to search for.",
                },
            ],
            example: r#"{"command": "search_sequence", "workspace": "demo", "text": "abc"}"#,
            result_type: "SearchResult { complete, token, query_exhausted, partial }",
            notes: "Returns complete=true if the entire text exists as a single token. Check partial for prefix/postfix matches.",
        },
        // -- Insert --
        CommandHelp {
            name: "insert_first_match",
            category: "insert",
            summary: "Insert a pattern using token references (advanced).",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "query",
                    param_type: "array of TokenRef",
                    required: true,
                    description: r#"Sequence of token references to insert."#,
                },
            ],
            example: r#"{"command": "insert_first_match", "workspace": "demo", "query": [{"index": 0}, {"index": 1}]}"#,
            result_type: "InsertResult { token, already_existed }",
            notes: "Advanced — prefer insert_sequence for text.",
        },
        CommandHelp {
            name: "insert_sequence",
            category: "insert",
            summary: "Insert a text string into the graph, creating all necessary hierarchy.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "text",
                    param_type: "string",
                    required: true,
                    description: "Text to insert.",
                },
            ],
            example: r#"{"command": "insert_sequence", "workspace": "demo", "text": "hello"}"#,
            result_type: "InsertResult { token: TokenInfo, already_existed: bool }",
            notes: "The primary insertion command. Atoms must exist first (use add_atoms). Returns already_existed=true if the exact sequence was already in the graph.",
        },
        CommandHelp {
            name: "insert_sequences",
            category: "insert",
            summary: "Insert multiple text strings at once.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "texts",
                    param_type: "array of string (set)",
                    required: true,
                    description: "Set of texts to insert (duplicates ignored).",
                },
            ],
            example: r#"{"command": "insert_sequences", "workspace": "demo", "texts": ["hello", "world"]}"#,
            result_type: "InsertResultList { results: [InsertResult] }",
            notes: "More efficient than calling insert_sequence repeatedly. Order is not guaranteed.",
        },
        // -- Read --
        CommandHelp {
            name: "read_pattern",
            category: "read",
            summary: "Read the full tree structure rooted at a vertex.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "index",
                    param_type: "integer",
                    required: true,
                    description: "Root vertex index to read from.",
                },
            ],
            example: r#"{"command": "read_pattern", "workspace": "demo", "index": 5}"#,
            result_type: "ReadResult { root: TokenInfo, text: string, tree: ReadNode }",
            notes: "Returns the hierarchical decomposition of the token into its children.",
        },
        CommandHelp {
            name: "read_as_text",
            category: "read",
            summary: "Read the text representation of a vertex (concatenation of its leaf atoms).",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "index",
                    param_type: "integer",
                    required: true,
                    description: "Vertex index to read.",
                },
            ],
            example: r#"{"command": "read_as_text", "workspace": "demo", "index": 5}"#,
            result_type: "Text { text: string }",
            notes: "The simplest way to see what text a vertex represents.",
        },
        CommandHelp {
            name: "read_sequence",
            category: "read",
            summary: "Read a text sequence through the graph (auto-creates atoms, builds decomposition).",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "text",
                    param_type: "string",
                    required: true,
                    description: "The text to read through the graph.",
                },
            ],
            example: r#"{"command": "read_sequence", "workspace": "demo", "text": "hello world"}"#,
            result_type: "ReadResult { root: TokenInfo, text: string, tree: ReadNode }",
            notes: "Auto-creates atoms for unknown characters. Returns the largest-match decomposition of the text.",
        },
        CommandHelp {
            name: "read_file",
            category: "read",
            summary: "Read a file's contents through the graph.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "path",
                    param_type: "string",
                    required: true,
                    description: "Path to the file to read.",
                },
            ],
            example: r#"{"command": "read_file", "workspace": "demo", "path": "/tmp/data.txt"}"#,
            result_type: "ReadResult { root: TokenInfo, text: string, tree: ReadNode }",
            notes: "Reads the file to a string, then processes it like read_sequence.",
        },
    ]
}