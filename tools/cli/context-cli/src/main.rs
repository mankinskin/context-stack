//! `context-cli`: standalone command-line tool on the context API.
//!
//! Every command builds a [`Request`] and runs it on a [`Backend`]
//! (in process: [`LocalBackend`]); output is the `Response` JSON
//! (`--format json`) or a text rendering of the result.

mod args;
mod backend;
#[cfg(any(feature = "server", feature = "mcp"))]
mod daemon;
mod error;
mod input;
#[cfg(feature = "mcp")]
mod mcp;
mod output;
mod progress;
#[cfg(feature = "server")]
mod serve;
mod shell;

use std::{
    fs,
    io::{
        IsTerminal,
        Read,
        Write,
    },
    path::Path,
    process::ExitCode,
};

use base64::Engine as _;
use clap::{
    ArgMatches,
    CommandFactory,
    FromArgMatches,
};
use context_api::{
    Command,
    CommandResult,
    Event,
    Outcome,
    Request,
    Response,
    Topic,
    client::RemoteBackend,
    types::{
        ExportFormat,
        Gate,
        ImportData,
        Limits,
        SavePolicy,
        SearchOutcome,
        VertexRef,
    },
};

use crate::{
    args::{
        Cli,
        CliCommand,
        ExportFormatArg,
        InputArgs,
        OutputFormat,
        TopCommand,
    },
    backend::{
        Backend,
        LocalBackend,
        Selection,
    },
    error::{
        CliError,
        EXIT_COMMAND,
        EXIT_OK,
        exit_code,
    },
};

/// Native `.ctx` files start with this magic.
const NATIVE_MAGIC: &[u8; 4] = b"CTXG";

fn main() -> ExitCode {
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let sub = matches.subcommand().map(|(_, m)| m);
    let code = match &cli.command {
        TopCommand::Graph(command) => run(&cli, command, sub),
        TopCommand::Shell(args) => shell::run(&cli, args),
        #[cfg(feature = "server")]
        TopCommand::Serve(args) => serve::run(args.clone()),
        #[cfg(feature = "mcp")]
        TopCommand::Mcp(args) => mcp::run(&cli, args),
    };
    ExitCode::from(code)
}

/// What to do with a successful result besides printing it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Post {
    None,
    Search {
        fail_if_missing: bool,
    },
    Show {
        output: Option<std::path::PathBuf>,
        fenced: bool,
    },
    Export {
        output: Option<std::path::PathBuf>,
    },
    Validate,
}

#[derive(Debug)]
struct Prepared {
    command: Command,
    post: Post,
    /// Bytes of read input (progress for large reads).
    input_bytes: usize,
}

/// A request id unlikely to collide with other clients' (matches progress
/// events of a remote read to this process).
fn request_id() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or_default();
    (u64::from(std::process::id()) << 32) | u64::from(nanos)
}

/// Run a command remotely, showing read progress from the server's events
/// when `progress`. The response carries no id (as a local one).
fn execute_remote(
    remote: &RemoteBackend,
    command: Command,
    progress: bool,
) -> Response {
    let id = progress.then(request_id);
    let watcher = id.zip(command.graph()).and_then(|(id, graph)| {
        let sink = progress::sink();
        remote
            .watch(graph, &[Topic::ReadProgress], move |event| {
                if let Event::ReadProgress { request, .. } = &event
                    && request.is_none_or(|request| request == id)
                {
                    sink(&event);
                }
            })
            .ok()
    });
    let mut response = backend::execute_remote(remote, Request { id, command });
    drop(watcher);
    response.id = None;
    response
}

fn run(
    cli: &Cli,
    command: &CliCommand,
    sub: Option<&ArgMatches>,
) -> u8 {
    let format = cli.output_format();
    let stdin_tty = std::io::stdin().is_terminal();
    let prepared = match prepare(
        command,
        format,
        sub,
        &mut std::io::stdin().lock(),
        stdin_tty,
    ) {
        Ok(prepared) => prepared,
        Err(err) => return report_error(&err.response, err.exit, format),
    };
    let selection = Selection::choose(
        cli.local,
        cli.server.as_deref(),
        cli.token.as_deref(),
        &cli.store,
        prepared.command.graph(),
    );
    if !cli.quiet
        && let Some(note) = selection.note()
    {
        eprintln!("{note}");
    }
    let progress = !cli.quiet
        && prepared.input_bytes > progress::PROGRESS_THRESHOLD
        && std::io::stderr().is_terminal();
    let response = match selection {
        Selection::Local => {
            let backend = LocalBackend::new(
                cli.store.clone(),
                progress.then(progress::sink),
            );
            backend.execute(Request::new(prepared.command))
        },
        Selection::Remote { url, token, .. } => execute_remote(
            &RemoteBackend::new(url, token),
            prepared.command,
            progress,
        ),
    };
    if progress {
        progress::finish();
    }
    finish(command.name(), response, &prepared.post, format)
}

fn report_error(
    error: &context_api::ErrorResponse,
    exit: u8,
    format: OutputFormat,
) -> u8 {
    if format == OutputFormat::Json {
        output::print_json(&Response {
            id: None,
            outcome: Outcome::Error(error.clone()),
        });
    }
    output::print_error(error);
    exit
}

/// Build the API command of the CLI command (`format`: the effective
/// output format).
fn prepare(
    command: &CliCommand,
    format: OutputFormat,
    sub: Option<&ArgMatches>,
    stdin: &mut dyn Read,
    stdin_tty: bool,
) -> Result<Prepared, CliError> {
    let simple = |command| Prepared {
        command,
        post: Post::None,
        input_bytes: 0,
    };
    Ok(match command {
        CliCommand::New(a) => simple(Command::New {
            graph: a.graph.clone(),
            overwrite: a.force,
        }),
        CliCommand::Create(a) => {
            let inputs = read_inputs(&a.input, sub, stdin, stdin_tty, false)?;
            let input_bytes = input::total_bytes(&inputs);
            Prepared {
                command: Command::Create {
                    graph: a.graph.clone(),
                    inputs,
                    split: a.input.split.into(),
                    overwrite: a.force,
                },
                post: Post::None,
                input_bytes,
            }
        },
        CliCommand::Read(a) => {
            let inputs = read_inputs(&a.input, sub, stdin, stdin_tty, true)?;
            let input_bytes = input::total_bytes(&inputs);
            Prepared {
                command: Command::Read {
                    graph: a.graph.clone(),
                    inputs,
                    split: a.input.split.into(),
                    create: a.create,
                    save: SavePolicy::Auto,
                    steps: a.steps,
                    dry_run: a.dry_run,
                },
                post: Post::None,
                input_bytes,
            }
        },
        CliCommand::Search(a) => Prepared {
            command: Command::Search {
                graph: a.graph.clone(),
                query: a.query.clone(),
                view: a.view.into(),
                limits: Limits {
                    limit: a.limit,
                    context_width: a.width,
                    label_width: a.label_width,
                    max_depth: a.max_depth,
                    max_visited: None,
                },
            },
            post: Post::Search {
                fail_if_missing: a.fail_if_missing,
            },
            input_bytes: 0,
        },
        CliCommand::Show(a) => {
            let (show_format, fenced) = a.resolved_format();
            Prepared {
                command: Command::Render {
                    graph: a.graph.clone(),
                    format: show_format.into(),
                    root: a.root.as_deref().map(vertex_ref),
                    depth: a.depth,
                    gate: Gate {
                        max_vertices: a.max_vertices,
                        max_label_chars: None,
                        force: a.force,
                    },
                    limits: Limits::default(),
                },
                post: Post::Show {
                    output: a.output.clone(),
                    fenced,
                },
                input_bytes: 0,
            }
        },
        CliCommand::Vertex(a) => simple(Command::Vertex {
            graph: a.graph.clone(),
            vertex: vertex_ref(&a.vertex),
            limits: Limits {
                limit: a.limit,
                label_width: a.label_width,
                ..Limits::default()
            },
        }),
        CliCommand::Inputs(a) => simple(Command::Inputs {
            graph: a.graph.clone(),
            limits: Limits {
                limit: a.limit,
                ..Limits::default()
            },
        }),
        CliCommand::Info(a) => simple(Command::Info {
            graph: a.graph.clone(),
        }),
        CliCommand::Stats(a) => simple(Command::Stats {
            graph: a.graph.clone(),
        }),
        CliCommand::Validate(a) => Prepared {
            command: Command::Validate {
                graph: a.graph.clone(),
            },
            post: Post::Validate,
            input_bytes: 0,
        },
        CliCommand::Export(a) => {
            if a.format == ExportFormatArg::Native
                && a.output.is_none()
                && format == OutputFormat::Text
            {
                return Err(CliError::usage("native export is binary and is not written to stdout")
                    .with_hint("pass -o FILE, or --format json before the command for the base64 envelope"));
            }
            Prepared {
                command: Command::Export {
                    graph: a.graph.clone(),
                    format: match a.format {
                        ExportFormatArg::V3 => ExportFormat::InterchangeV3,
                        ExportFormatArg::Native => ExportFormat::Native,
                    },
                    gate: Gate {
                        force: a.force,
                        ..Gate::default()
                    },
                },
                post: Post::Export {
                    output: a.output.clone(),
                },
                input_bytes: 0,
            }
        },
        CliCommand::Import(a) => simple(Command::Import {
            graph: a.graph.clone(),
            data: import_data(&a.file)?,
            overwrite: a.force,
        }),
        CliCommand::List(_) => simple(Command::List),
        CliCommand::Delete(a) => simple(Command::Delete {
            graph: a.graph.clone(),
        }),
    })
}

fn read_inputs(
    args: &InputArgs,
    sub: Option<&ArgMatches>,
    stdin: &mut dyn Read,
    stdin_tty: bool,
    required: bool,
) -> Result<Vec<context_api::types::Input>, CliError> {
    let sources = input::with_implicit_stdin(args.sources(sub), stdin_tty);
    if sources.is_empty() && required {
        return Err(CliError::usage("no INPUT given").with_hint(
            "pass text arguments, -f FILE, or pipe text into stdin ('-')",
        ));
    }
    input::resolve(&sources, &args.options(), stdin)
}

/// `#123` is an index, anything else is text.
fn vertex_ref(vertex: &str) -> VertexRef {
    vertex
        .strip_prefix('#')
        .and_then(|index| index.parse().ok())
        .map(VertexRef::Index)
        .unwrap_or_else(|| VertexRef::Text(vertex.to_string()))
}

/// Native `.ctx` bytes (by magic) or a v3 interchange JSON document.
fn import_data(path: &Path) -> Result<ImportData, CliError> {
    let bytes = fs::read(path).map_err(|e| {
        CliError::io(format!("cannot read '{}': {e}", path.display()))
    })?;
    if bytes.starts_with(NATIVE_MAGIC) {
        return Ok(ImportData::Native {
            base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        });
    }
    let document = String::from_utf8(bytes).map_err(|_| {
        CliError::io(format!(
            "'{}' is neither a native .ctx file nor UTF-8 JSON",
            path.display()
        ))
    })?;
    Ok(ImportData::InterchangeV3 { document })
}

/// Handle the response: file outputs, printing and the exit code.
fn finish(
    name: &str,
    response: Response,
    post: &Post,
    format: OutputFormat,
) -> u8 {
    let result = match &response.outcome {
        Outcome::Ok(result) => result,
        Outcome::Error(error) =>
            return report_error(error, exit_code(error.kind), format),
    };
    if let Err(err) = write_outputs(result, post) {
        return report_error(&err.response, err.exit, format);
    }
    match format {
        OutputFormat::Json => output::print_json(&response),
        OutputFormat::Text => {
            let text = match (post, result) {
                (
                    Post::Show {
                        output: Some(path), ..
                    },
                    CommandResult::Render(render),
                ) => format!(
                    "wrote {} vertices, {} edges to {}",
                    render.vertices,
                    render.edges,
                    path.display()
                ),
                (
                    Post::Export { output: Some(path) },
                    CommandResult::Export(export),
                ) => format!(
                    "exported {} bytes to {}",
                    export.bytes,
                    path.display()
                ),
                _ => output::text(name, result),
            };
            if !text.is_empty() {
                println!("{text}");
            }
        },
    }
    match (post, result) {
        (
            Post::Search {
                fail_if_missing: true,
            },
            CommandResult::Search(search),
        ) if matches!(
            search.outcome,
            SearchOutcome::NotFound { .. } | SearchOutcome::Partial { .. }
        ) =>
            EXIT_COMMAND,
        (Post::Validate, CommandResult::Validation(report))
            if !report.valid =>
            EXIT_COMMAND,
        _ => EXIT_OK,
    }
}

/// Write `show -o` and `export -o` files.
fn write_outputs(
    result: &CommandResult,
    post: &Post,
) -> Result<(), CliError> {
    match (post, result) {
        (
            Post::Show {
                output: Some(path),
                fenced,
            },
            CommandResult::Render(render),
        ) => {
            let mut content = match (&render.text, &render.graph) {
                (Some(text), _) => text.clone(),
                (None, Some(graph)) => serde_json::to_string_pretty(graph)
                    .expect("render graphs serialize"),
                (None, None) => String::new(),
            };
            if *fenced {
                content = format!("```mermaid\n{}\n```", content.trim_end());
            }
            if !content.ends_with('\n') {
                content.push('\n');
            }
            write_atomic(path, content.as_bytes())
        },
        (
            Post::Export { output: Some(path) },
            CommandResult::Export(export),
        ) => {
            let bytes = match export.format {
                ExportFormat::Native =>
                    base64::engine::general_purpose::STANDARD
                        .decode(&export.data)
                        .map_err(|e| {
                            CliError::io(format!(
                                "invalid base64 in native export: {e}"
                            ))
                        })?,
                ExportFormat::InterchangeV3 => export.data.clone().into_bytes(),
            };
            write_atomic(path, &bytes)
        },
        _ => Ok(()),
    }
}

/// Write via a temporary sibling file and rename.
fn write_atomic(
    path: &Path,
    bytes: &[u8],
) -> Result<(), CliError> {
    let io = |e: std::io::Error| {
        CliError::io(format!("cannot write '{}': {e}", path.display()))
    };
    let name = path.file_name().ok_or_else(|| {
        CliError::invalid(format!("invalid output path '{}'", path.display()))
    })?;
    let tmp = path.with_file_name(format!(
        ".{}.tmp{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(io)
}

#[cfg(test)]
mod tests {
    use context_api::types::{
        Input,
        Split,
    };

    use super::*;

    fn prepare_args(
        args: &[&str],
        stdin: &str,
    ) -> Result<Prepared, CliError> {
        let matches = Cli::command().try_get_matches_from(args).unwrap();
        let cli = Cli::from_arg_matches(&matches).unwrap();
        let sub = matches.subcommand().map(|(_, m)| m);
        let TopCommand::Graph(command) = &cli.command else {
            panic!("graph command expected")
        };
        prepare(
            command,
            cli.output_format(),
            sub,
            &mut stdin.as_bytes(),
            false,
        )
    }

    fn labels_and_texts(command: &Command) -> Vec<(String, Option<String>)> {
        let (Command::Read { inputs, .. } | Command::Create { inputs, .. }) =
            command
        else {
            panic!("{command:?}")
        };
        inputs
            .iter()
            .map(|input| match input {
                Input::Text { text, label } => (text.clone(), label.clone()),
                Input::File { .. } => panic!("file input"),
            })
            .collect()
    }

    #[test]
    fn inputs_keep_command_line_order() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.txt");
        fs::write(&file, "file").unwrap();
        let file = file.to_str().unwrap();
        let prepared = prepare_args(
            &[
                "context-cli",
                "read",
                "g",
                "-f",
                file,
                "one",
                "-",
                "-t",
                "-two",
                "three",
            ],
            "stdin",
        )
        .unwrap();
        let texts: Vec<_> = labels_and_texts(&prepared.command)
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        assert_eq!(texts, ["file", "one", "stdin", "-two", "three"]);
    }

    #[test]
    fn split_is_passed_through() {
        let prepared = prepare_args(
            &["context-cli", "create", "g", "a\nb", "--split", "lines"],
            "",
        )
        .unwrap();
        assert!(matches!(
            prepared.command,
            Command::Create {
                split: Split::Lines,
                ..
            }
        ));
        let prepared = prepare_args(
            &["context-cli", "read", "g", "x", "--split", "paragraphs"],
            "",
        )
        .unwrap();
        assert!(matches!(
            prepared.command,
            Command::Read {
                split: Split::Paragraphs,
                ..
            }
        ));
    }

    #[test]
    fn implicit_stdin_when_no_input() {
        let prepared =
            prepare_args(&["context-cli", "read", "g"], "piped").unwrap();
        assert_eq!(
            labels_and_texts(&prepared.command),
            [("piped".to_string(), Some("<stdin>".to_string()))]
        );
    }

    #[test]
    fn vertex_refs() {
        assert_eq!(vertex_ref("#12"), VertexRef::Index(12));
        assert_eq!(vertex_ref("#x"), VertexRef::Text("#x".into()));
        assert_eq!(vertex_ref("abc"), VertexRef::Text("abc".into()));
    }

    #[test]
    fn show_format_from_extension() {
        let prepared =
            prepare_args(&["context-cli", "show", "g", "-o", "x.md"], "")
                .unwrap();
        assert!(matches!(prepared.post, Post::Show { fenced: true, .. }));
        assert!(matches!(
            prepared.command,
            Command::Render {
                format: context_api::types::RenderFormat::Mermaid,
                ..
            }
        ));
        let prepared =
            prepare_args(&["context-cli", "show", "g", "-o", "x.dot"], "")
                .unwrap();
        assert!(matches!(
            prepared.command,
            Command::Render {
                format: context_api::types::RenderFormat::Dot,
                ..
            }
        ));
    }
}
