//! Interactive REPL for the context-engine CLI.
//!
//! Provides a readline-based interactive loop with:
//! - A prompt that shows the currently active workspace (if any)
//! - Tab-friendly command parsing (space-separated tokens)
//! - Help text listing all available commands
//! - History support via `rustyline`

use std::collections::HashSet;

use rustyline::{
    DefaultEditor,
    error::ReadlineError,
};

use context_api::{
    commands::{
        Command,
        execute,
        execute_traced,
    },
    tracing_capture::CaptureConfig,
    types::TokenRef,
    workspace::manager::WorkspaceManager,
};

use crate::output;

/// Run the interactive REPL loop.
///
/// The REPL maintains a "current workspace" that is used as the default
/// target for graph commands (atom, pattern, vertex, etc.). Workspace
/// lifecycle commands (create, open, close, save, list, delete) can be
/// used to manage the current workspace.
pub fn run(manager: &mut WorkspaceManager) {
    println!("Context Engine REPL (type 'help' for commands, 'quit' to exit)");

    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Error initializing readline: {e}");
            return;
        },
    };

    let mut current_workspace: Option<String> = None;
    let mut tracing_enabled = false;

    loop {
        let prompt = match &current_workspace {
            Some(name) => format!("({name})> "),
            None => "> ".to_string(),
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(line);

                match line {
                    "quit" | "exit" => break,
                    "help" | "?" => print_help(),
                    _ => {
                        execute_repl_line(
                            manager,
                            &mut current_workspace,
                            &mut tracing_enabled,
                            line,
                        );
                    },
                }
            },
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                break;
            },
            Err(ReadlineError::Eof) => {
                println!("^D");
                break;
            },
            Err(err) => {
                eprintln!("Error: {err}");
                break;
            },
        }
    }

    println!("Goodbye.");
}

/// Parse a string as a `TokenRef`.
///
/// If it parses as a `usize`, it becomes `TokenRef::Index`.
/// Otherwise, it becomes `TokenRef::Label`.
fn parse_token_ref(s: &str) -> TokenRef {
    match s.parse::<usize>() {
        Ok(n) => TokenRef::Index(n),
        Err(_) => TokenRef::Label(s.to_string()),
    }
}

/// Parse and execute a single REPL line.
fn execute_repl_line(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    line: &str,
) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    match parts[0] {
        // -- Workspace lifecycle --------------------------------------------
        "create" => {
            if let Some(name) = parts.get(1) {
                match execute_and_print(
                    manager,
                    Command::CreateWorkspace {
                        name: name.to_string(),
                    },
                    *tracing_enabled,
                    current_ws.as_deref(),
                ) {
                    Ok(_) => {
                        // Automatically set as current workspace
                        *current_ws = Some(name.to_string());
                        println!("(workspace '{}' is now active)", name);
                    },
                    Err(()) => {}, // error already printed
                }
            } else {
                eprintln!("Usage: create <name>");
            }
        },

        "create-ngrams" => {
            if parts.len() < 3 {
                eprintln!(
                    "Usage: create-ngrams <name> [--timeout <seconds>] <text>"
                );
                return;
            }

            let name = parts[1];
            let mut timeout_secs: u64 = 60;
            let mut text_start = 2usize;

            if parts.get(2) == Some(&"--timeout") {
                let Some(timeout_str) = parts.get(3) else {
                    eprintln!(
                        "Usage: create-ngrams <name> [--timeout <seconds>] <text>"
                    );
                    return;
                };
                match timeout_str.parse::<u64>() {
                    Ok(v) if v > 0 => timeout_secs = v,
                    _ => {
                        eprintln!(
                            "Error: timeout must be a positive integer seconds"
                        );
                        return;
                    },
                }
                text_start = 4;
            }

            if text_start >= parts.len() {
                eprintln!(
                    "Usage: create-ngrams <name> [--timeout <seconds>] <text>"
                );
                return;
            }

            let text = parts[text_start..].join(" ");
            match execute_and_print(
                manager,
                Command::CreateWorkspaceFromNgrams {
                    name: name.to_string(),
                    text,
                    timeout_secs,
                },
                *tracing_enabled,
                current_ws.as_deref(),
            ) {
                Ok(_) => {
                    *current_ws = Some(name.to_string());
                    println!("(workspace '{}' is now active)", name);
                },
                Err(()) => {},
            }
        },

        "open" =>
            if let Some(name) = parts.get(1) {
                match execute_and_print(
                    manager,
                    Command::OpenWorkspace {
                        name: name.to_string(),
                    },
                    *tracing_enabled,
                    current_ws.as_deref(),
                ) {
                    Ok(_) => {
                        *current_ws = Some(name.to_string());
                        println!("(workspace '{}' is now active)", name);
                    },
                    Err(()) => {},
                }
            } else {
                eprintln!("Usage: open <name>");
            },

        "close" => {
            let name = parts
                .get(1)
                .map(|s| s.to_string())
                .or_else(|| current_ws.clone());

            if let Some(name) = name {
                match execute_and_print(
                    manager,
                    Command::CloseWorkspace { name: name.clone() },
                    *tracing_enabled,
                    current_ws.as_deref(),
                ) {
                    Ok(_) =>
                        if current_ws.as_deref() == Some(&name) {
                            *current_ws = None;
                        },
                    Err(()) => {},
                }
            } else {
                eprintln!(
                    "Usage: close [<name>]  (defaults to current workspace)"
                );
            }
        },

        "save" => {
            let name = parts
                .get(1)
                .map(|s| s.to_string())
                .or_else(|| current_ws.clone());

            if let Some(name) = name {
                execute_and_print(
                    manager,
                    Command::SaveWorkspace { name },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            } else {
                eprintln!(
                    "Usage: save [<name>]  (defaults to current workspace)"
                );
            }
        },

        "list" => {
            execute_and_print(
                manager,
                Command::ListWorkspaces,
                *tracing_enabled,
                current_ws.as_deref(),
            )
            .ok();
        },

        "delete" =>
            if let Some(name) = parts.get(1) {
                match execute_and_print(
                    manager,
                    Command::DeleteWorkspace {
                        name: name.to_string(),
                    },
                    *tracing_enabled,
                    current_ws.as_deref(),
                ) {
                    Ok(_) =>
                        if current_ws.as_deref() == Some(*name) {
                            *current_ws = None;
                        },
                    Err(()) => {},
                }
            } else {
                eprintln!("Usage: delete <name>");
            },

        // -- Graph commands (require current workspace) ---------------------
        "atom" => {
            if let Some(ws) = require_workspace(current_ws) {
                if let Some(chars_str) = parts.get(1) {
                    if chars_str.len() == 1 {
                        // Single atom
                        let ch = chars_str.chars().next().unwrap();
                        execute_and_print(
                            manager,
                            Command::AddAtom { workspace: ws, ch },
                            *tracing_enabled,
                            current_ws.as_deref(),
                        )
                        .ok();
                    } else {
                        // Multiple atoms
                        let char_vec: Vec<char> = chars_str.chars().collect();
                        execute_and_print(
                            manager,
                            Command::AddAtoms {
                                workspace: ws,
                                chars: char_vec,
                            },
                            *tracing_enabled,
                            current_ws.as_deref(),
                        )
                        .ok();
                    }
                } else {
                    eprintln!(
                        "Usage: atom <chars>  (e.g. 'atom a' or 'atom abcde')"
                    );
                }
            }
        },

        "pattern" =>
            if let Some(ws) = require_workspace(current_ws) {
                if let Some(atoms_str) = parts.get(1) {
                    let atom_chars: Vec<char> = atoms_str.chars().collect();
                    execute_and_print(
                        manager,
                        Command::AddSimplePattern {
                            workspace: ws,
                            atoms: atom_chars,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                } else {
                    eprintln!(
                        "Usage: pattern <chars>  (e.g. 'pattern abc' creates pattern from atoms a, b, c)"
                    );
                }
            },

        "vertex" =>
            if let Some(ws) = require_workspace(current_ws) {
                if let Some(index_str) = parts.get(1) {
                    match index_str.parse::<usize>() {
                        Ok(index) => {
                            execute_and_print(
                                manager,
                                Command::GetVertex {
                                    workspace: ws,
                                    index,
                                },
                                *tracing_enabled,
                                current_ws.as_deref(),
                            )
                            .ok();
                        },
                        Err(_) => {
                            eprintln!(
                                "Error: '{}' is not a valid index",
                                index_str
                            );
                        },
                    }
                } else {
                    eprintln!("Usage: vertex <index>");
                }
            },

        "vertices" =>
            if let Some(ws) = require_workspace(current_ws) {
                execute_and_print(
                    manager,
                    Command::ListVertices { workspace: ws },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            },

        "atoms" =>
            if let Some(ws) = require_workspace(current_ws) {
                execute_and_print(
                    manager,
                    Command::ListAtoms { workspace: ws },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            },

        // -- Search commands (Phase 2) --------------------------------------
        "search" =>
            if let Some(ws) = require_workspace(current_ws) {
                if parts.len() < 2 {
                    eprintln!(
                        "Usage: search <text>  or  search <ref1> <ref2> ..."
                    );
                } else if parts.len() == 2 && parts[1].parse::<usize>().is_err()
                {
                    // Single non-numeric argument → search as text sequence
                    let text = parts[1].to_string();
                    execute_and_print(
                        manager,
                        Command::SearchSequence {
                            workspace: ws,
                            text,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                } else {
                    // Multiple arguments or single numeric → search as pattern with TokenRefs
                    let query: Vec<TokenRef> =
                        parts[1..].iter().map(|s| parse_token_ref(s)).collect();
                    execute_and_print(
                        manager,
                        Command::SearchPattern {
                            workspace: ws,
                            query,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                }
            },

        // -- Insert commands (Phase 2) --------------------------------------
        "insert" =>
            if let Some(ws) = require_workspace(current_ws) {
                if parts.len() < 2 {
                    eprintln!(
                        "Usage: insert <text>  (inserts the text as a sequence)"
                    );
                } else {
                    // Join all remaining parts as the text to insert
                    let text = parts[1..].join(" ");
                    execute_and_print(
                        manager,
                        Command::InsertSequence {
                            workspace: ws,
                            text,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                }
            },

        "insert-match" =>
            if let Some(ws) = require_workspace(current_ws) {
                if parts.len() < 3 {
                    eprintln!(
                        "Usage: insert-match <ref1> <ref2> ...  (at least 2 token refs)"
                    );
                } else {
                    let query: Vec<TokenRef> =
                        parts[1..].iter().map(|s| parse_token_ref(s)).collect();
                    execute_and_print(
                        manager,
                        Command::InsertFirstMatch {
                            workspace: ws,
                            query,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                }
            },

        "insert-bulk" =>
            if let Some(ws) = require_workspace(current_ws) {
                if parts.len() < 2 {
                    eprintln!(
                        "Usage: insert-bulk <text1> <text2> ...  (each arg is a sequence)"
                    );
                } else {
                    let texts: HashSet<String> =
                        parts[1..].iter().map(|s| s.to_string()).collect();
                    execute_and_print(
                        manager,
                        Command::InsertSequences {
                            workspace: ws,
                            texts,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                }
            },

        // -- Read commands (Phase 2) ----------------------------------------
        "read" =>
            if let Some(ws) = require_workspace(current_ws) {
                if parts.len() < 2 {
                    eprintln!(
                        "Usage: read <index>    Read vertex by index\n\
                         \x20      read <text>     Read text sequence through graph\n\
                         \x20      read --file <path>  Read file contents through graph"
                    );
                } else if parts[1] == "--file" {
                    // File input mode
                    if let Some(path) = parts.get(2) {
                        execute_and_print(
                            manager,
                            Command::ReadFile {
                                workspace: ws,
                                path: path.to_string(),
                            },
                            *tracing_enabled,
                            current_ws.as_deref(),
                        )
                        .ok();
                    } else {
                        eprintln!("Usage: read --file <path>");
                    }
                } else if parts.len() == 2 && parts[1].parse::<usize>().is_ok()
                {
                    // Single numeric argument → ReadPattern (backwards compatible)
                    let index = parts[1].parse::<usize>().unwrap();
                    execute_and_print(
                        manager,
                        Command::ReadPattern {
                            workspace: ws,
                            index,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                } else {
                    // Non-numeric or multi-word → ReadSequence
                    let text = parts[1..].join(" ");
                    execute_and_print(
                        manager,
                        Command::ReadSequence {
                            workspace: ws,
                            text,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                }
            },

        "text" =>
            if let Some(ws) = require_workspace(current_ws) {
                if let Some(index_str) = parts.get(1) {
                    match index_str.parse::<usize>() {
                        Ok(index) => {
                            execute_and_print(
                                manager,
                                Command::ReadAsText {
                                    workspace: ws,
                                    index,
                                },
                                *tracing_enabled,
                                current_ws.as_deref(),
                            )
                            .ok();
                        },
                        Err(_) => {
                            eprintln!(
                                "Error: '{}' is not a valid index",
                                index_str
                            );
                        },
                    }
                } else {
                    eprintln!("Usage: text <index>");
                }
            },

        // -- Validation (Phase 2) -------------------------------------------
        "validate" =>
            if let Some(ws) = require_workspace(current_ws) {
                execute_and_print(
                    manager,
                    Command::ValidateGraph { workspace: ws },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            },

        // -- Show commands --------------------------------------------------
        "show" =>
            if let Some(ws) = require_workspace(current_ws) {
                if let Some(arg) = parts.get(1) {
                    match arg.parse::<usize>() {
                        Ok(index) => {
                            execute_and_print(
                                manager,
                                Command::ShowVertex {
                                    workspace: ws,
                                    index,
                                },
                                *tracing_enabled,
                                current_ws.as_deref(),
                            )
                            .ok();
                        },
                        Err(_) => {
                            eprintln!(
                                "Error: '{}' is not a valid vertex index",
                                arg
                            );
                        },
                    }
                } else {
                    execute_and_print(
                        manager,
                        Command::ShowGraph { workspace: ws },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                }
            },

        "render" | "render-graph" =>
            if let Some(ws) = require_workspace(current_ws) {
                let ascii_dag =
                    parts.iter().skip(1).any(|arg| *arg == "--ascii-dag");
                execute_and_print(
                    manager,
                    Command::RenderAsciiGraph {
                        workspace: ws,
                        ascii_dag,
                    },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            },

        // -- Debug / introspection ------------------------------------------
        "snapshot" =>
            if let Some(ws) = require_workspace(current_ws) {
                execute_and_print(
                    manager,
                    Command::GetSnapshot { workspace: ws },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            },

        "stats" =>
            if let Some(ws) = require_workspace(current_ws) {
                execute_and_print(
                    manager,
                    Command::GetStatistics { workspace: ws },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            },

        // -- Informational --------------------------------------------------
        "ws" | "workspace" => match current_ws {
            Some(name) => println!("Current workspace: {name}"),
            None => println!(
                "No workspace is currently active. Use 'create' or 'open'."
            ),
        },

        "use" | "switch" =>
            if let Some(name) = parts.get(1) {
                if manager.is_open(name) {
                    *current_ws = Some(name.to_string());
                    println!("Switched to workspace '{}'", name);
                } else {
                    eprintln!(
                        "Workspace '{}' is not open. Use 'open {}' first.",
                        name, name
                    );
                }
            } else {
                eprintln!(
                    "Usage: use <name>  (switch to an already-open workspace)"
                );
            },

        // -- Tracing toggle -------------------------------------------------
        "trace" => match parts.get(1).map(|s| *s) {
            Some("on") => {
                *tracing_enabled = true;
                println!(
                    "Tracing enabled. Graph commands will write .log files."
                );
            },
            Some("off") => {
                *tracing_enabled = false;
                println!("Tracing disabled.");
            },
            Some("status") | None => {
                let status = if *tracing_enabled { "on" } else { "off" };
                println!("Tracing is {status}.");
            },
            Some(other) => {
                eprintln!(
                    "Unknown trace option: '{other}'. Use: trace on|off|status"
                );
            },
        },

        // -- Log commands ---------------------------------------------------
        "logs" =>
            if let Some(ws) = require_workspace(current_ws) {
                let pattern = parts.get(1).map(|s| s.to_string());
                execute_and_print(
                    manager,
                    Command::ListLogs {
                        workspace: ws,
                        pattern,
                        limit: 100,
                    },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            },

        "log" =>
            if let Some(ws) = require_workspace(current_ws) {
                if let Some(filename) = parts.get(1) {
                    let filter = parts.get(2).map(|s| s.to_string());
                    execute_and_print(
                        manager,
                        Command::GetLog {
                            workspace: ws,
                            filename: filename.to_string(),
                            filter,
                            limit: 100,
                            offset: 0,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                } else {
                    eprintln!("Usage: log <filename> [filter]");
                }
            },

        "query" =>
            if let Some(ws) = require_workspace(current_ws) {
                if parts.len() >= 3 {
                    let filename = parts[1].to_string();
                    let query = parts[2..].join(" ");
                    execute_and_print(
                        manager,
                        Command::QueryLog {
                            workspace: ws,
                            filename,
                            query,
                            limit: 100,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                } else {
                    eprintln!("Usage: query <filename> <jq-expression>");
                }
            },

        "analyze" =>
            if let Some(ws) = require_workspace(current_ws) {
                if let Some(filename) = parts.get(1) {
                    execute_and_print(
                        manager,
                        Command::AnalyzeLog {
                            workspace: ws,
                            filename: filename.to_string(),
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                } else {
                    eprintln!("Usage: analyze <filename>");
                }
            },

        "search-logs" =>
            if let Some(ws) = require_workspace(current_ws) {
                if parts.len() >= 2 {
                    let query = parts[1..].join(" ");
                    execute_and_print(
                        manager,
                        Command::SearchLogs {
                            workspace: ws,
                            query,
                            limit_per_file: 10,
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                } else {
                    eprintln!("Usage: search-logs <jq-expression>");
                }
            },

        "delete-log" =>
            if let Some(ws) = require_workspace(current_ws) {
                if let Some(filename) = parts.get(1) {
                    execute_and_print(
                        manager,
                        Command::DeleteLog {
                            workspace: ws,
                            filename: filename.to_string(),
                        },
                        *tracing_enabled,
                        current_ws.as_deref(),
                    )
                    .ok();
                } else {
                    eprintln!("Usage: delete-log <filename>");
                }
            },

        "clean-logs" =>
            if let Some(ws) = require_workspace(current_ws) {
                let older_than_days =
                    parts.get(1).and_then(|s| s.parse::<u32>().ok());
                execute_and_print(
                    manager,
                    Command::DeleteLogs {
                        workspace: ws,
                        older_than_days,
                    },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            },

        // -- Compare (Phase 3.2) -------------------------------------------
        "compare" => {
            if parts.len() < 3 {
                eprintln!(
                    "Usage: compare <workspace-a> <workspace-b> [--mode full|subset]"
                );
            } else {
                let ws_a = parts[1].to_string();
                let ws_b = parts[2].to_string();

                // Parse optional --mode flag
                let mode = {
                    let mut m = context_api::types::CompareMode::Full;
                    let mut i = 3usize;
                    while i < parts.len() {
                        if parts[i] == "--mode" {
                            if let Some(mode_str) = parts.get(i + 1) {
                                m = mode_str
                                    .parse::<context_api::types::CompareMode>()
                                    .unwrap_or_else(|e| {
                                        eprintln!("Warning: {e}. Defaulting to 'full'.");
                                        context_api::types::CompareMode::Full
                                    });
                                i += 2;
                                continue;
                            }
                        }
                        i += 1;
                    }
                    m
                };

                // Auto-open workspaces if not already open
                let _ = execute(
                    manager,
                    Command::OpenWorkspace { name: ws_a.clone() },
                );
                let _ = execute(
                    manager,
                    Command::OpenWorkspace { name: ws_b.clone() },
                );

                execute_and_print(
                    manager,
                    Command::CompareWorkspaces {
                        workspace_a: ws_a,
                        workspace_b: ws_b,
                        mode,
                    },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            }
        },

        "compare-vertex" => {
            if parts.len() < 5 {
                eprintln!(
                    "Usage: compare-vertex <ws-a> <index-a> <ws-b> <index-b>"
                );
            } else {
                let ws_a = parts[1].to_string();
                let index_a = match parts[2].parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!(
                            "Error: '{}' is not a valid vertex index",
                            parts[2]
                        );
                        return;
                    },
                };
                let ws_b = parts[3].to_string();
                let index_b = match parts[4].parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!(
                            "Error: '{}' is not a valid vertex index",
                            parts[4]
                        );
                        return;
                    },
                };

                // Auto-open workspaces if not already open
                let _ = execute(
                    manager,
                    Command::OpenWorkspace { name: ws_a.clone() },
                );
                let _ = execute(
                    manager,
                    Command::OpenWorkspace { name: ws_b.clone() },
                );

                execute_and_print(
                    manager,
                    Command::CompareVertices {
                        workspace_a: ws_a,
                        index_a,
                        workspace_b: ws_b,
                        index_b,
                    },
                    *tracing_enabled,
                    current_ws.as_deref(),
                )
                .ok();
            }
        },

        _ => {
            eprintln!(
                "Unknown command: '{}'. Type 'help' for available commands.",
                parts[0]
            );
        },
    }
}

/// Execute a command, optionally with tracing, and print the result.
///
/// When `tracing_enabled` is `true` and a workspace name is available, the
/// command is executed inside a tracing capture span that writes structured
/// JSON events to a log file. A short summary line is printed to stderr
/// after the normal output so the user knows a trace was recorded.
///
/// Returns `Ok(())` on success, `Err(())` if the command returned an error
/// (which is printed to stderr).
fn execute_and_print(
    manager: &mut WorkspaceManager,
    cmd: Command,
    tracing_enabled: bool,
    workspace_name: Option<&str>,
) -> Result<(), ()> {
    if tracing_enabled {
        if let Some(ws_name) = workspace_name {
            if let Ok(log_dir) = manager.log_dir(ws_name) {
                let config = CaptureConfig {
                    enabled: true,
                    log_dir,
                    level: "TRACE".to_string(),
                };
                match execute_traced(manager, cmd, Some(&config)) {
                    Ok((result, trace_summary)) => {
                        output::print_command_result(&result);
                        if let Some(summary) = trace_summary {
                            eprintln!(
                                "📝 Trace: {} ({} events, {}ms)",
                                summary.log_file,
                                summary.entry_count,
                                summary.duration_ms,
                            );
                        }
                        return Ok(());
                    },
                    Err(e) => {
                        eprintln!("Error: {e}");
                        return Err(());
                    },
                }
            }
        }
    }
    match execute(manager, cmd) {
        Ok(result) => {
            output::print_command_result(&result);
            Ok(())
        },
        Err(e) => {
            eprintln!("Error: {e}");
            Err(())
        },
    }
}

/// Ensure a current workspace is set, printing an error if not.
/// Returns the workspace name if available.
fn require_workspace(current_ws: &Option<String>) -> Option<String> {
    match current_ws {
        Some(name) => Some(name.clone()),
        None => {
            eprintln!(
                "No workspace is active. Use 'create <name>' or 'open <name>' first."
            );
            None
        },
    }
}

/// Print the help text listing all available REPL commands.
fn print_help() {
    println!();
    println!("Workspace commands:");
    println!(
        "  create <name>        Create a new workspace and set it as active"
    );
    println!(
        "  create-ngrams <name> [--timeout <sec>] <text>  Create workspace from ngrams text (default timeout: 60s)"
    );
    println!(
        "  open <name>          Open a workspace from disk and set it as active"
    );
    println!("  close [<name>]       Close the current (or named) workspace");
    println!(
        "  save [<name>]        Save the current (or named) workspace to disk"
    );
    println!("  list                 List all workspaces (open and on-disk)");
    println!("  delete <name>        Delete a workspace from disk");
    println!(
        "  use <name>           Switch active workspace (must already be open)"
    );
    println!("  ws                   Show the currently active workspace");
    println!();
    println!("Graph commands (require an active workspace):");
    println!("  atom <chars>         Add atom(s): 'atom a' or 'atom abcde'");
    println!(
        "  pattern <chars>      Add a simple pattern from atoms: 'pattern abc'"
    );
    println!("  vertex <index>       Show detailed vertex information");
    println!("  vertices             List all vertices");
    println!("  atoms                List all atoms");
    println!();
    println!("Search commands:");
    println!(
        "  search <text>        Search for a text sequence (e.g. 'search hello')"
    );
    println!(
        "  search <r1> <r2> ... Search by token refs (numbers=indices, strings=labels)"
    );
    println!();
    println!("Insert commands:");
    println!(
        "  insert <text>        Insert a text sequence (e.g. 'insert hello world')"
    );
    println!(
        "  insert-match <refs>  Insert by token refs (numbers=indices, strings=labels)"
    );
    println!(
        "  insert-bulk <t1> ... Bulk insert multiple sequences (each arg is one)"
    );
    println!();
    println!("Read commands:");
    println!("  read <index>         Read a vertex as a decomposition tree");
    println!("  read <text>          Read a text sequence through the graph");
    println!("  read --file <path>   Read a file's contents through the graph");
    println!("  text <index>         Read a vertex as concatenated leaf text");
    println!();
    println!("Debug commands:");
    println!("  validate             Validate graph integrity");
    println!("  snapshot             Print graph snapshot as JSON");
    println!("  stats                Print graph statistics");
    println!(
        "  show                 Show the entire graph (all vertices with children & parents)"
    );
    println!(
        "  show <index>         Show a single vertex with its children and parents"
    );
    println!(
        "  render [--ascii-dag]  Render graph (default aligned grammar layout)"
    );
    println!();
    println!("Tracing commands:");
    println!("  trace on|off|status  Toggle or check per-command tracing");
    println!();
    println!("Log commands (require an active workspace):");
    println!("  logs [pattern]       List log files (optionally filtered)");
    println!(
        "  log <file> [filter]  Read a log file (optionally filter by level/message)"
    );
    println!("  query <file> <jq>    Query a log file with a JQ expression");
    println!("  analyze <file>       Show log analysis (stats, spans, errors)");
    println!("  search-logs <jq>     Search across all log files");
    println!("  delete-log <file>    Delete a specific log file");
    println!(
        "  clean-logs [days]    Delete all logs (or only those older than N days)"
    );
    println!();
    println!("Compare commands:");
    println!("  compare <ws-a> <ws-b> [--mode full|subset]");
    println!(
        "                       Compare two workspace graphs (diff or subset check)"
    );
    println!("  compare-vertex <ws-a> <idx-a> <ws-b> <idx-b>");
    println!(
        "                       Compare two individual vertices across workspaces"
    );
    println!();
    println!("General:");
    println!("  help / ?             Show this help");
    println!("  quit / exit          Exit the REPL");
    println!();
}
