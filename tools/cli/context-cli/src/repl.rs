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
        let prompt = repl_prompt(&current_workspace);
        match read_repl_input(&mut rl, &prompt) {
            ReplInput::Line(line) => {
                if handle_repl_input(
                    manager,
                    &mut rl,
                    &mut current_workspace,
                    &mut tracing_enabled,
                    line,
                ) {
                    break;
                }
            },
            ReplInput::Break => break,
        }
    }

    println!("Goodbye.");
}

enum ReplInput {
    Line(String),
    Break,
}

type ReplHandler =
    fn(&mut WorkspaceManager, &mut Option<String>, &mut bool, &[&str]);

struct ReplCommandDef {
    names: &'static [&'static str],
    handler: ReplHandler,
}

const COMMAND_HANDLERS: &[ReplCommandDef] = &[
    ReplCommandDef {
        names: &["create"],
        handler: handle_create,
    },
    ReplCommandDef {
        names: &["create-ngrams"],
        handler: handle_create_ngrams,
    },
    ReplCommandDef {
        names: &["open"],
        handler: handle_open,
    },
    ReplCommandDef {
        names: &["close"],
        handler: handle_close,
    },
    ReplCommandDef {
        names: &["save"],
        handler: handle_save,
    },
    ReplCommandDef {
        names: &["list"],
        handler: handle_list,
    },
    ReplCommandDef {
        names: &["delete"],
        handler: handle_delete,
    },
    ReplCommandDef {
        names: &["atom"],
        handler: handle_atom,
    },
    ReplCommandDef {
        names: &["pattern"],
        handler: handle_pattern,
    },
    ReplCommandDef {
        names: &["vertex"],
        handler: handle_vertex,
    },
    ReplCommandDef {
        names: &["vertices"],
        handler: handle_vertices,
    },
    ReplCommandDef {
        names: &["atoms"],
        handler: handle_atoms,
    },
    ReplCommandDef {
        names: &["search"],
        handler: handle_search,
    },
    ReplCommandDef {
        names: &["insert"],
        handler: handle_insert,
    },
    ReplCommandDef {
        names: &["insert-match"],
        handler: handle_insert_match,
    },
    ReplCommandDef {
        names: &["insert-bulk"],
        handler: handle_insert_bulk,
    },
    ReplCommandDef {
        names: &["read"],
        handler: handle_read,
    },
    ReplCommandDef {
        names: &["text"],
        handler: handle_text,
    },
    ReplCommandDef {
        names: &["validate"],
        handler: handle_validate,
    },
    ReplCommandDef {
        names: &["show"],
        handler: handle_show,
    },
    ReplCommandDef {
        names: &["render", "render-graph"],
        handler: handle_render,
    },
    ReplCommandDef {
        names: &["snapshot"],
        handler: handle_snapshot,
    },
    ReplCommandDef {
        names: &["stats"],
        handler: handle_stats,
    },
    ReplCommandDef {
        names: &["ws", "workspace"],
        handler: handle_workspace_status,
    },
    ReplCommandDef {
        names: &["use", "switch"],
        handler: handle_use,
    },
    ReplCommandDef {
        names: &["trace"],
        handler: handle_trace,
    },
    ReplCommandDef {
        names: &["logs"],
        handler: handle_logs,
    },
    ReplCommandDef {
        names: &["log"],
        handler: handle_log,
    },
    ReplCommandDef {
        names: &["query"],
        handler: handle_query,
    },
    ReplCommandDef {
        names: &["analyze"],
        handler: handle_analyze,
    },
    ReplCommandDef {
        names: &["search-logs"],
        handler: handle_search_logs,
    },
    ReplCommandDef {
        names: &["delete-log"],
        handler: handle_delete_log,
    },
    ReplCommandDef {
        names: &["clean-logs"],
        handler: handle_clean_logs,
    },
    ReplCommandDef {
        names: &["compare"],
        handler: handle_compare,
    },
    ReplCommandDef {
        names: &["compare-vertex"],
        handler: handle_compare_vertex,
    },
];

fn repl_prompt(current_workspace: &Option<String>) -> String {
    match current_workspace {
        Some(name) => format!("({name})> "),
        None => "> ".to_string(),
    }
}

fn read_repl_input(
    rl: &mut DefaultEditor,
    prompt: &str,
) -> ReplInput {
    match rl.readline(prompt) {
        Ok(line) => ReplInput::Line(line),
        Err(ReadlineError::Interrupted) => {
            println!("^C");
            ReplInput::Break
        },
        Err(ReadlineError::Eof) => {
            println!("^D");
            ReplInput::Break
        },
        Err(err) => {
            eprintln!("Error: {err}");
            ReplInput::Break
        },
    }
}

fn handle_repl_input(
    manager: &mut WorkspaceManager,
    rl: &mut DefaultEditor,
    current_workspace: &mut Option<String>,
    tracing_enabled: &mut bool,
    line: String,
) -> bool {
    let line = line.trim();
    if line.is_empty() {
        return false;
    }

    let _ = rl.add_history_entry(line);
    match line {
        "quit" | "exit" => true,
        "help" | "?" => {
            print_help();
            false
        },
        _ => {
            execute_repl_line(
                manager,
                current_workspace,
                tracing_enabled,
                line,
            );
            false
        },
    }
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

fn find_repl_handler(command: &str) -> Option<ReplHandler> {
    COMMAND_HANDLERS
        .iter()
        .find(|entry| entry.names.contains(&command))
        .map(|entry| entry.handler)
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

    if let Some(handler) = find_repl_handler(parts[0]) {
        handler(manager, current_ws, tracing_enabled, &parts);
    } else {
        eprintln!(
            "Unknown command: '{}'. Type 'help' for available commands.",
            parts[0]
        );
    }
}

fn activate_workspace(
    current_ws: &mut Option<String>,
    name: &str,
) {
    *current_ws = Some(name.to_string());
    println!("(workspace '{}' is now active)", name);
}

fn clear_current_workspace_if_matches(
    current_ws: &mut Option<String>,
    name: &str,
) {
    if current_ws.as_deref() == Some(name) {
        *current_ws = None;
    }
}

fn execute_and_activate_workspace(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: bool,
    command: Command,
    name: &str,
) {
    if execute_and_print(
        manager,
        command,
        tracing_enabled,
        current_ws.as_deref(),
    )
    .is_ok()
    {
        activate_workspace(current_ws, name);
    }
}

fn execute_in_current_workspace<F>(
    manager: &mut WorkspaceManager,
    current_ws: &Option<String>,
    tracing_enabled: bool,
    build_command: F,
) where
    F: FnOnce(String) -> Command,
{
    if let Some(workspace) = require_workspace(current_ws) {
        execute_and_print(
            manager,
            build_command(workspace),
            tracing_enabled,
            current_ws.as_deref(),
        )
        .ok();
    }
}

fn parse_index_arg(
    value: &str,
    error_label: &str,
) -> Option<usize> {
    match value.parse::<usize>() {
        Ok(index) => Some(index),
        Err(_) => {
            eprintln!("Error: '{}' is not a valid {error_label}", value);
            None
        },
    }
}

fn open_workspace_if_needed(
    manager: &mut WorkspaceManager,
    name: &str,
) {
    let _ = execute(
        manager,
        Command::OpenWorkspace {
            name: name.to_string(),
        },
    );
}

fn parse_compare_mode(parts: &[&str]) -> context_api::types::CompareMode {
    let mut mode = context_api::types::CompareMode::Full;
    let mut index = 3usize;
    while index < parts.len() {
        if parts[index] == "--mode" {
            if let Some(mode_str) = parts.get(index + 1) {
                mode = mode_str
                    .parse::<context_api::types::CompareMode>()
                    .unwrap_or_else(|error| {
                        eprintln!("Warning: {error}. Defaulting to 'full'.");
                        context_api::types::CompareMode::Full
                    });
                index += 2;
                continue;
            }
        }
        index += 1;
    }
    mode
}

fn handle_create(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if let Some(name) = parts.get(1) {
        execute_and_activate_workspace(
            manager,
            current_ws,
            *tracing_enabled,
            Command::CreateWorkspace {
                name: name.to_string(),
            },
            name,
        );
    } else {
        eprintln!("Usage: create <name>");
    }
}

fn handle_create_ngrams(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 3 {
        eprintln!("Usage: create-ngrams <name> [--timeout <seconds>] <text>");
        return;
    }

    let name = parts[1];
    let mut timeout_secs = 60u64;
    let mut text_start = 2usize;
    if parts.get(2) == Some(&"--timeout") {
        let Some(timeout_str) = parts.get(3) else {
            eprintln!(
                "Usage: create-ngrams <name> [--timeout <seconds>] <text>"
            );
            return;
        };
        match timeout_str.parse::<u64>() {
            Ok(value) if value > 0 => timeout_secs = value,
            _ => {
                eprintln!("Error: timeout must be a positive integer seconds");
                return;
            },
        }
        text_start = 4;
    }

    if text_start >= parts.len() {
        eprintln!("Usage: create-ngrams <name> [--timeout <seconds>] <text>");
        return;
    }

    execute_and_activate_workspace(
        manager,
        current_ws,
        *tracing_enabled,
        Command::CreateWorkspaceFromNgrams {
            name: name.to_string(),
            text: parts[text_start..].join(" "),
            timeout_secs,
        },
        name,
    );
}

fn handle_open(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if let Some(name) = parts.get(1) {
        execute_and_activate_workspace(
            manager,
            current_ws,
            *tracing_enabled,
            Command::OpenWorkspace {
                name: name.to_string(),
            },
            name,
        );
    } else {
        eprintln!("Usage: open <name>");
    }
}

fn handle_close(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let name = parts
        .get(1)
        .map(|value| value.to_string())
        .or_else(|| current_ws.clone());
    if let Some(name) = name {
        if execute_and_print(
            manager,
            Command::CloseWorkspace { name: name.clone() },
            *tracing_enabled,
            current_ws.as_deref(),
        )
        .is_ok()
        {
            clear_current_workspace_if_matches(current_ws, &name);
        }
    } else {
        eprintln!("Usage: close [<name>]  (defaults to current workspace)");
    }
}

fn handle_save(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let name = parts
        .get(1)
        .map(|value| value.to_string())
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
        eprintln!("Usage: save [<name>]  (defaults to current workspace)");
    }
}

fn handle_list(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    _parts: &[&str],
) {
    execute_and_print(
        manager,
        Command::ListWorkspaces,
        *tracing_enabled,
        current_ws.as_deref(),
    )
    .ok();
}

fn handle_delete(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if let Some(name) = parts.get(1) {
        if execute_and_print(
            manager,
            Command::DeleteWorkspace {
                name: name.to_string(),
            },
            *tracing_enabled,
            current_ws.as_deref(),
        )
        .is_ok()
        {
            clear_current_workspace_if_matches(current_ws, name);
        }
    } else {
        eprintln!("Usage: delete <name>");
    }
}

fn handle_atom(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let Some(chars) = parts.get(1) else {
        eprintln!("Usage: atom <chars>  (e.g. 'atom a' or 'atom abcde')");
        return;
    };

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        if chars.len() == 1 {
            Command::AddAtom {
                workspace: ws,
                ch: chars.chars().next().unwrap(),
            }
        } else {
            Command::AddAtoms {
                workspace: ws,
                chars: chars.chars().collect(),
            }
        }
    });
}

fn handle_pattern(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let Some(atoms) = parts.get(1) else {
        eprintln!(
            "Usage: pattern <chars>  (e.g. 'pattern abc' creates pattern from atoms a, b, c)"
        );
        return;
    };

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::AddSimplePattern {
            workspace: ws,
            atoms: atoms.chars().collect(),
        }
    });
}

fn handle_vertex(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let Some(index_str) = parts.get(1) else {
        eprintln!("Usage: vertex <index>");
        return;
    };
    let Some(index) = parse_index_arg(index_str, "index") else {
        return;
    };

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::GetVertex {
            workspace: ws,
            index,
        }
    });
}

fn handle_vertices(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    _parts: &[&str],
) {
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::ListVertices { workspace: ws }
    });
}

fn handle_atoms(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    _parts: &[&str],
) {
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::ListAtoms { workspace: ws }
    });
}

fn handle_search(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 2 {
        eprintln!("Usage: search <text>  or  search <ref1> <ref2> ...");
        return;
    }

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        if parts.len() == 2 && parts[1].parse::<usize>().is_err() {
            Command::SearchSequence {
                workspace: ws,
                text: parts[1].to_string(),
            }
        } else {
            Command::SearchPattern {
                workspace: ws,
                query: parts[1..]
                    .iter()
                    .map(|part| parse_token_ref(part))
                    .collect(),
            }
        }
    });
}

fn handle_insert(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 2 {
        eprintln!("Usage: insert <text>  (inserts the text as a sequence)");
        return;
    }

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::InsertSequence {
            workspace: ws,
            text: parts[1..].join(" "),
        }
    });
}

fn handle_insert_match(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 3 {
        eprintln!(
            "Usage: insert-match <ref1> <ref2> ...  (at least 2 token refs)"
        );
        return;
    }

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::InsertFirstMatch {
            workspace: ws,
            query: parts[1..]
                .iter()
                .map(|part| parse_token_ref(part))
                .collect(),
        }
    });
}

fn handle_insert_bulk(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 2 {
        eprintln!(
            "Usage: insert-bulk <text1> <text2> ...  (each arg is a sequence)"
        );
        return;
    }

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::InsertSequences {
            workspace: ws,
            texts: parts[1..]
                .iter()
                .map(|part| part.to_string())
                .collect::<HashSet<_>>(),
        }
    });
}

fn handle_read(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 2 {
        eprintln!(
            "Usage: read <index>    Read vertex by index\n\
             \x20      read <text>     Read text sequence through graph\n\
             \x20      read --file <path>  Read file contents through graph"
        );
        return;
    }

    if parts[1] == "--file" {
        let Some(path) = parts.get(2) else {
            eprintln!("Usage: read --file <path>");
            return;
        };
        execute_in_current_workspace(
            manager,
            current_ws,
            *tracing_enabled,
            |ws| Command::ReadFile {
                workspace: ws,
                path: path.to_string(),
            },
        );
        return;
    }

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        if parts.len() == 2 {
            if let Ok(index) = parts[1].parse::<usize>() {
                return Command::ReadPattern {
                    workspace: ws,
                    index,
                };
            }
        }

        Command::ReadSequence {
            workspace: ws,
            text: parts[1..].join(" "),
        }
    });
}

fn handle_text(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let Some(index_str) = parts.get(1) else {
        eprintln!("Usage: text <index>");
        return;
    };
    let Some(index) = parse_index_arg(index_str, "index") else {
        return;
    };

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::ReadAsText {
            workspace: ws,
            index,
        }
    });
}

fn handle_validate(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    _parts: &[&str],
) {
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::ValidateGraph { workspace: ws }
    });
}

fn handle_show(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let index = match parts.get(1) {
        Some(arg) => match parse_index_arg(arg, "vertex index") {
            Some(index) => Some(index),
            None => return,
        },
        None => None,
    };

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        match index {
            Some(index) => Command::ShowVertex {
                workspace: ws,
                index,
            },
            None => Command::ShowGraph { workspace: ws },
        }
    });
}

fn handle_render(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let ascii_dag = parts.iter().skip(1).any(|arg| *arg == "--ascii-dag");
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::RenderAsciiGraph {
            workspace: ws,
            ascii_dag,
        }
    });
}

fn handle_snapshot(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    _parts: &[&str],
) {
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::GetSnapshot { workspace: ws }
    });
}

fn handle_stats(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    _parts: &[&str],
) {
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::GetStatistics { workspace: ws }
    });
}

fn handle_workspace_status(
    _manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    _tracing_enabled: &mut bool,
    _parts: &[&str],
) {
    match current_ws {
        Some(name) => println!("Current workspace: {name}"),
        None => println!(
            "No workspace is currently active. Use 'create' or 'open'."
        ),
    }
}

fn handle_use(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    _tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let Some(name) = parts.get(1) else {
        eprintln!("Usage: use <name>  (switch to an already-open workspace)");
        return;
    };

    if manager.is_open(name) {
        *current_ws = Some(name.to_string());
        println!("Switched to workspace '{}'", name);
    } else {
        eprintln!(
            "Workspace '{}' is not open. Use 'open {}' first.",
            name, name
        );
    }
}

fn handle_trace(
    _manager: &mut WorkspaceManager,
    _current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    match parts.get(1).copied() {
        Some("on") => {
            *tracing_enabled = true;
            println!("Tracing enabled. Graph commands will write .log files.");
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
    }
}

fn handle_logs(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let pattern = parts.get(1).map(|value| value.to_string());
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::ListLogs {
            workspace: ws,
            pattern,
            limit: 100,
        }
    });
}

fn handle_log(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let Some(filename) = parts.get(1) else {
        eprintln!("Usage: log <filename> [filter]");
        return;
    };

    let filter = parts.get(2).map(|value| value.to_string());
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::GetLog {
            workspace: ws,
            filename: filename.to_string(),
            filter,
            limit: 100,
            offset: 0,
        }
    });
}

fn handle_query(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 3 {
        eprintln!("Usage: query <filename> <jq-expression>");
        return;
    }

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::QueryLog {
            workspace: ws,
            filename: parts[1].to_string(),
            query: parts[2..].join(" "),
            limit: 100,
        }
    });
}

fn handle_analyze(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let Some(filename) = parts.get(1) else {
        eprintln!("Usage: analyze <filename>");
        return;
    };

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::AnalyzeLog {
            workspace: ws,
            filename: filename.to_string(),
        }
    });
}

fn handle_search_logs(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 2 {
        eprintln!("Usage: search-logs <jq-expression>");
        return;
    }

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::SearchLogs {
            workspace: ws,
            query: parts[1..].join(" "),
            limit_per_file: 10,
        }
    });
}

fn handle_delete_log(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let Some(filename) = parts.get(1) else {
        eprintln!("Usage: delete-log <filename>");
        return;
    };

    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::DeleteLog {
            workspace: ws,
            filename: filename.to_string(),
        }
    });
}

fn handle_clean_logs(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    let older_than_days =
        parts.get(1).and_then(|value| value.parse::<u32>().ok());
    execute_in_current_workspace(manager, current_ws, *tracing_enabled, |ws| {
        Command::DeleteLogs {
            workspace: ws,
            older_than_days,
        }
    });
}

fn handle_compare(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 3 {
        eprintln!(
            "Usage: compare <workspace-a> <workspace-b> [--mode full|subset]"
        );
        return;
    }

    let workspace_a = parts[1].to_string();
    let workspace_b = parts[2].to_string();
    open_workspace_if_needed(manager, &workspace_a);
    open_workspace_if_needed(manager, &workspace_b);
    execute_and_print(
        manager,
        Command::CompareWorkspaces {
            workspace_a,
            workspace_b,
            mode: parse_compare_mode(parts),
        },
        *tracing_enabled,
        current_ws.as_deref(),
    )
    .ok();
}

fn handle_compare_vertex(
    manager: &mut WorkspaceManager,
    current_ws: &mut Option<String>,
    tracing_enabled: &mut bool,
    parts: &[&str],
) {
    if parts.len() < 5 {
        eprintln!("Usage: compare-vertex <ws-a> <index-a> <ws-b> <index-b>");
        return;
    }

    let Some(index_a) = parse_index_arg(parts[2], "vertex index") else {
        return;
    };
    let Some(index_b) = parse_index_arg(parts[4], "vertex index") else {
        return;
    };

    let workspace_a = parts[1].to_string();
    let workspace_b = parts[3].to_string();
    open_workspace_if_needed(manager, &workspace_a);
    open_workspace_if_needed(manager, &workspace_b);
    execute_and_print(
        manager,
        Command::CompareVertices {
            workspace_a,
            index_a,
            workspace_b,
            index_b,
        },
        *tracing_enabled,
        current_ws.as_deref(),
    )
    .ok();
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
