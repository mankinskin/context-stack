use crate::server::{
    CommandHelp,
    ParamHelp,
};

pub(super) fn workspace_commands() -> Vec<CommandHelp> {
    vec![
        // -- Workspace lifecycle --
        CommandHelp {
            name: "create_workspace",
            category: "workspace",
            summary: "Create a new empty workspace.",
            parameters: &[ParamHelp {
                name: "name",
                param_type: "string",
                required: true,
                description: "Name for the new workspace.",
            }],
            example: r#"{"command": "create_workspace", "name": "my-graph"}"#,
            result_type: "WorkspaceInfo",
            notes: "Fails if a workspace with the same name already exists in memory.",
        },
        CommandHelp {
            name: "open_workspace",
            category: "workspace",
            summary: "Open a previously saved workspace from disk.",
            parameters: &[ParamHelp {
                name: "name",
                param_type: "string",
                required: true,
                description: "Name of the workspace to open.",
            }],
            example: r#"{"command": "open_workspace", "name": "my-graph"}"#,
            result_type: "WorkspaceInfo",
            notes: "Loads from <base_dir>/.context-engine/<name>/. Fails if not found on disk.",
        },
        CommandHelp {
            name: "close_workspace",
            category: "workspace",
            summary: "Close a workspace (remove from memory). Does NOT auto-save.",
            parameters: &[ParamHelp {
                name: "name",
                param_type: "string",
                required: true,
                description: "Name of the workspace to close.",
            }],
            example: r#"{"command": "close_workspace", "name": "my-graph"}"#,
            result_type: "Ok",
            notes: "Remember to save_workspace first if you want to keep changes!",
        },
        CommandHelp {
            name: "save_workspace",
            category: "workspace",
            summary: "Save a workspace to disk.",
            parameters: &[ParamHelp {
                name: "name",
                param_type: "string",
                required: true,
                description: "Name of the workspace to save.",
            }],
            example: r#"{"command": "save_workspace", "name": "my-graph"}"#,
            result_type: "Ok",
            notes: "Persists to <base_dir>/.context-engine/<name>/. Always save before closing.",
        },
        CommandHelp {
            name: "list_workspaces",
            category: "workspace",
            summary: "List all workspaces currently open in memory.",
            parameters: &[],
            example: r#"{"command": "list_workspaces"}"#,
            result_type: "WorkspaceInfoList",
            notes: "Only shows in-memory workspaces. Saved-but-closed workspaces won't appear.",
        },
        CommandHelp {
            name: "delete_workspace",
            category: "workspace",
            summary: "Delete a workspace from memory and disk.",
            parameters: &[ParamHelp {
                name: "name",
                param_type: "string",
                required: true,
                description: "Name of the workspace to delete.",
            }],
            example: r#"{"command": "delete_workspace", "name": "old-graph"}"#,
            result_type: "Ok",
            notes: "Irreversible. Removes both in-memory state and on-disk files.",
        },
        // -- Atoms --
        CommandHelp {
            name: "add_atom",
            category: "atoms",
            summary: "Add a single character atom to the graph.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "ch",
                    param_type: "char",
                    required: true,
                    description: "The character to add (single character string).",
                },
            ],
            example: r#"{"command": "add_atom", "workspace": "demo", "ch": "a"}"#,
            result_type: "AtomInfo { index, ch }",
            notes: "Idempotent — adding the same character twice returns the existing atom.",
        },
        CommandHelp {
            name: "add_atoms",
            category: "atoms",
            summary: "Add multiple character atoms at once.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "chars",
                    param_type: "array of char",
                    required: true,
                    description: "Characters to add (e.g., [\"a\", \"b\", \"c\"]).",
                },
            ],
            example: r#"{"command": "add_atoms", "workspace": "demo", "chars": ["a","b","c","d","e"]}"#,
            result_type: "AtomInfoList { atoms: [AtomInfo] }",
            notes: "Recommended before insert_sequence — add all unique characters your text uses.",
        },
        CommandHelp {
            name: "get_atom",
            category: "atoms",
            summary: "Look up an atom by its character.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "ch",
                    param_type: "char",
                    required: true,
                    description: "The character to look up.",
                },
            ],
            example: r#"{"command": "get_atom", "workspace": "demo", "ch": "a"}"#,
            result_type: "OptionalAtomInfo { atom: AtomInfo | null }",
            notes: "Returns null if the character hasn't been added as an atom.",
        },
        CommandHelp {
            name: "list_atoms",
            category: "atoms",
            summary: "List all atoms in the workspace.",
            parameters: &[ParamHelp {
                name: "workspace",
                param_type: "string",
                required: true,
                description: "Target workspace name.",
            }],
            example: r#"{"command": "list_atoms", "workspace": "demo"}"#,
            result_type: "AtomInfoList { atoms: [AtomInfo] }",
            notes: "Atoms are the leaf nodes of the graph — each represents one character.",
        },
        // -- Patterns --
        CommandHelp {
            name: "add_simple_pattern",
            category: "patterns",
            summary: "Create a pattern (compound vertex) from a sequence of atom characters.",
            parameters: &[
                ParamHelp {
                    name: "workspace",
                    param_type: "string",
                    required: true,
                    description: "Target workspace name.",
                },
                ParamHelp {
                    name: "atoms",
                    param_type: "array of char",
                    required: true,
                    description: "Ordered characters forming the pattern (e.g., [\"a\", \"b\"]).",
                },
            ],
            example: r#"{"command": "add_simple_pattern", "workspace": "demo", "atoms": ["a","b"]}"#,
            result_type: "PatternInfo { index, label, width, children }",
            notes: "Low-level. Prefer insert_sequence for text — it handles atom creation and hierarchy.",
        },
        CommandHelp {
            name: "get_vertex",
            category: "patterns",
            summary: "Get detailed information about a vertex by its index.",
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
                    description: "Vertex index to look up.",
                },
            ],
            example: r#"{"command": "get_vertex", "workspace": "demo", "index": 5}"#,
            result_type: "OptionalVertexInfo { vertex: VertexInfo | null }",
            notes: "Returns label, width, is_atom flag, children indices, and parent count.",
        },
        CommandHelp {
            name: "list_vertices",
            category: "patterns",
            summary: "List all vertices (atoms and patterns) in the graph.",
            parameters: &[ParamHelp {
                name: "workspace",
                param_type: "string",
                required: true,
                description: "Target workspace name.",
            }],
            example: r#"{"command": "list_vertices", "workspace": "demo"}"#,
            result_type: "TokenInfoList { tokens: [TokenInfo] }",
            notes: "Can be large — use get_statistics first to check vertex_count.",
        },
    ]
}
