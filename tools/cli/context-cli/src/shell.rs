//! `context-cli shell`: an interactive command loop (plan 04 §4.4).
//!
//! A line is a graph command of the CLI without the program name, parsed by
//! the same clap definitions ([`ShellLine`]). With a default graph (`shell
//! GRAPH` or `use GRAPH`), a command that misses a required argument is
//! retried with the default graph inserted as its first argument, so
//! `search "fox"` means `search GRAPH "fox"`. Shell settings start with
//! `:`. On a terminal the shell uses rustyline (history, completion, events
//! printed above the prompt); otherwise it reads commands line by line
//! without prompt or history (scripts).

use std::{
    collections::{
        HashMap,
        HashSet,
    },
    io::{
        BufRead,
        IsTerminal,
    },
    path::PathBuf,
    sync::{
        Arc,
        Mutex,
        MutexGuard,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
};

use clap::{
    CommandFactory,
    FromArgMatches,
    error::ErrorKind as ClapErrorKind,
};
use context_api::{
    CommandResult,
    Event,
    EventSink,
    Outcome,
    Request,
    Response,
    SaveMode,
    Session,
    client::{
        EventWatcher,
        RemoteBackend,
    },
};
use rustyline::{
    Config,
    Editor,
    ExternalPrinter as _,
    completion::Completer,
    error::ReadlineError,
    highlight::Highlighter,
    hint::Hinter,
    history::FileHistory,
    validate::Validator,
};

use crate::{
    args::{
        Cli,
        CliCommand,
        OutputFormat,
        ShellArgs,
        ShellLine,
    },
    backend::{
        self,
        Selection,
    },
    error::{
        CliError,
        EXIT_IO,
        EXIT_OK,
    },
    progress,
};

const SHELL_COMMANDS: &[&str] = &[
    ":format", ":server", ":local", ":events", ":save", ":status", ":help",
    ":quit", "use", "exit", "quit",
];

const HELP: &str = "\
Commands are the context-cli graph commands without the program name
(`help` lists them, `help search` shows one). With a default graph, the
graph argument may be omitted:  search \"fox\" --view occurrences

  use <GRAPH>               set the default graph
  :format text|json         output format
  :server <URL>             run commands on a server (token: --token / CONTEXT_TOKEN)
  :local                    run commands in process
  :events on|off            print graph_changed / saved events of other clients (remote)
  :save on-read|manual      when the local shell saves (always on exit)
  :status                   show the current settings
  :help                     this text
  :quit, exit, Ctrl-D       leave the shell";

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// State shared with event callbacks (other threads).
struct Shared {
    /// Prints a message line (above the prompt on a terminal).
    printer: Mutex<Box<dyn FnMut(String) + Send>>,
    /// Show read progress of the running command.
    progress: AtomicBool,
    progress_sink: EventSink,
    /// Print change events of other clients.
    events: AtomicBool,
    own: Mutex<OwnChanges>,
}

/// What this shell changed, to tell its own events from other clients'.
#[derive(Default)]
struct OwnChanges {
    /// Graph of the running mutating command: its events are ours.
    busy: Option<String>,
    request: Option<u64>,
    inputs: HashSet<(String, u32)>,
    generations: HashSet<(String, u64)>,
}

impl Shared {
    fn print(
        &self,
        message: String,
    ) {
        (lock(&self.printer))(message);
    }

    /// Handle an event of the remote subscription.
    fn on_event(
        &self,
        event: Event,
    ) {
        let own = lock(&self.own);
        match &event {
            Event::ReadProgress { request, .. } => {
                if self.progress.load(Ordering::SeqCst)
                    && (request.is_none() || *request == own.request)
                {
                    (self.progress_sink)(&event);
                }
                return;
            },
            Event::GraphChanged {
                graph,
                generation,
                new_inputs,
                ..
            } => {
                let ours = own.busy.as_deref() == Some(graph.as_str())
                    || own.generations.contains(&(graph.clone(), *generation))
                    || new_inputs.iter().any(|input| {
                        own.inputs.contains(&(graph.clone(), *input))
                    });
                if ours {
                    return;
                }
            },
            Event::Saved { graph, generation } => {
                if own.busy.as_deref() == Some(graph.as_str())
                    || own.generations.contains(&(graph.clone(), *generation))
                {
                    return;
                }
            },
            Event::Lagged { .. } => {},
        }
        drop(own);
        if self.events.load(Ordering::SeqCst) {
            self.print(event_line(&event));
        }
    }
}

/// One line describing an event.
fn event_line(event: &Event) -> String {
    match event {
        Event::GraphChanged {
            graph,
            generation,
            vertex_count,
            new_inputs,
        } => format!(
            "[event] graph_changed {graph}: generation {generation}, {vertex_count} vertices, new inputs {new_inputs:?}"
        ),
        Event::Saved { graph, generation } =>
            format!("[event] saved {graph}: generation {generation}"),
        Event::Lagged { skipped } =>
            format!("[event] lagged: {skipped} events skipped"),
        Event::ReadProgress {
            graph,
            input,
            chars,
            total,
            ..
        } => format!(
            "[event] read_progress {graph}: input {input}, {chars}/{total} chars"
        ),
    }
}

enum Mode {
    Local(Session),
    Remote {
        remote: RemoteBackend,
        watcher: Option<EventWatcher>,
    },
    /// Between releasing one backend and setting up the next.
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Quit,
}

struct Shell {
    store: PathBuf,
    token: Option<String>,
    quiet: bool,
    format: OutputFormat,
    graph: Option<String>,
    save: SaveMode,
    mode: Mode,
    shared: Arc<Shared>,
    /// Exit code of the last failed command (scripts).
    last_error: u8,
}

pub fn run(
    cli: &Cli,
    args: &ShellArgs,
) -> u8 {
    let interactive = std::io::stdin().is_terminal();
    let selection = Selection::choose(
        cli.local,
        cli.server.as_deref(),
        cli.token.as_deref(),
        &cli.store,
        args.graph.as_deref(),
    );
    if !cli.quiet
        && let Some(note) = selection.note()
    {
        eprintln!("{note}");
    }
    let shared = Arc::new(Shared {
        printer: Mutex::new(Box::new(|message| println!("{message}"))),
        progress: AtomicBool::new(false),
        progress_sink: progress::sink(),
        events: AtomicBool::new(false),
        own: Mutex::new(OwnChanges::default()),
    });
    let mut shell = Shell {
        store: cli.store.clone(),
        token: cli.token.clone(),
        quiet: cli.quiet,
        format: cli.format.unwrap_or(OutputFormat::Text),
        graph: args.graph.clone(),
        save: SaveMode::OnWrite,
        mode: Mode::Local(local_session(
            cli.store.clone(),
            SaveMode::OnWrite,
            &shared,
        )),
        shared,
        last_error: EXIT_OK,
    };
    if let Selection::Remote { url, token, .. } = selection
        && let Err(err) = shell.connect(RemoteBackend::new(url, token))
    {
        return err.exit;
    }
    if interactive {
        shell.interactive();
    } else {
        shell.script(std::io::stdin().lock());
    }
    let closed = shell.close();
    if closed != EXIT_OK {
        closed
    } else {
        shell.last_error
    }
}

/// A local session keeping graphs open between commands.
fn local_session(
    store: PathBuf,
    save: SaveMode,
    shared: &Arc<Shared>,
) -> Session {
    let mut config = backend::local_config(store);
    config.keep_open = true;
    config.save = save;
    let mut session = Session::new(config);
    let shared = shared.clone();
    session.set_event_sink(Arc::new(move |event: &Event| {
        if shared.progress.load(Ordering::SeqCst) {
            (shared.progress_sink)(event);
        }
    }));
    session
}

/// `$XDG_STATE_HOME/context-cli/history` (default `~/.local/state`).
fn history_path() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|dir| !dir.is_empty())
                .map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join("context-cli").join("history"))
}

impl Shell {
    fn interactive(&mut self) {
        let config = Config::builder().auto_add_history(true).build();
        let mut editor: Editor<ShellHelper, FileHistory> =
            match Editor::with_config(config) {
                Ok(editor) => editor,
                Err(err) => {
                    eprintln!("error: cannot start the line editor: {err}");
                    self.last_error = EXIT_IO;
                    return;
                },
            };
        editor.set_helper(Some(ShellHelper::new()));
        let history = history_path();
        if let Some(path) = &history {
            let _ = editor.load_history(path);
        }
        if let Ok(mut printer) = editor.create_external_printer() {
            *lock(&self.shared.printer) = Box::new(move |message: String| {
                let _ = printer.print(format!("{message}\n"));
            });
        }
        loop {
            let prompt =
                format!("{}> ", self.graph.as_deref().unwrap_or("context"));
            match editor.readline(&prompt) {
                Ok(line) =>
                    if self.line(&line) == Flow::Quit {
                        break;
                    },
                Err(ReadlineError::Interrupted) => {},
                Err(ReadlineError::Eof) => break,
                Err(err) => {
                    eprintln!("error: {err}");
                    break;
                },
            }
        }
        if let Some(path) = &history {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = editor.save_history(path);
        }
    }

    fn script(
        &mut self,
        input: impl BufRead,
    ) {
        for line in input.lines() {
            let Ok(line) = line else {
                eprintln!("error: stdin is not valid UTF-8");
                self.last_error = EXIT_IO;
                return;
            };
            if self.line(&line) == Flow::Quit {
                return;
            }
        }
    }

    /// Release the backend; a local session saves its open graphs.
    fn close(&mut self) -> u8 {
        match std::mem::replace(&mut self.mode, Mode::Closed) {
            Mode::Local(session) => match session.shutdown() {
                Ok(()) => EXIT_OK,
                Err(err) => {
                    eprintln!("error: saving on exit failed: {err}");
                    EXIT_IO
                },
            },
            Mode::Remote { .. } | Mode::Closed => EXIT_OK,
        }
    }

    fn fail(
        &mut self,
        err: CliError,
    ) {
        crate::report_error(&err.response, err.exit, self.format);
        self.last_error = err.exit;
    }

    /// Run one line.
    fn line(
        &mut self,
        line: &str,
    ) -> Flow {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return Flow::Continue;
        }
        let words = match shell_words::split(line) {
            Ok(words) => words,
            Err(err) => {
                self.fail(CliError::usage(format!(
                    "cannot split the line: {err}"
                )));
                return Flow::Continue;
            },
        };
        match words[0].as_str() {
            "exit" | "quit" => Flow::Quit,
            "use" => {
                match words.as_slice() {
                    [_, graph] => {
                        self.graph = Some(graph.clone());
                        self.rewatch();
                    },
                    _ => self.fail(CliError::usage("usage: use <GRAPH>")),
                }
                Flow::Continue
            },
            word if word.starts_with(':') => self.setting(&words),
            _ => {
                self.command(&words);
                Flow::Continue
            },
        }
    }

    /// Parse with the CLI grammar; on a missing argument retry with the
    /// default graph inserted as the first argument.
    fn parse(
        &self,
        words: &[String],
    ) -> Result<(CliCommand, clap::ArgMatches), clap::Error> {
        let attempt = |words: &[String]| {
            let matches = ShellLine::command().try_get_matches_from(words)?;
            let line = ShellLine::from_arg_matches(&matches)?;
            let sub = matches
                .subcommand()
                .map(|(_, sub)| sub.clone())
                .unwrap_or_default();
            Ok::<_, clap::Error>((line.command, sub))
        };
        match (attempt(words), &self.graph) {
            (Err(err), Some(graph))
                if err.kind() == ClapErrorKind::MissingRequiredArgument =>
            {
                let mut with_graph = words.to_vec();
                with_graph.insert(1, graph.clone());
                attempt(&with_graph)
            },
            (result, _) => result,
        }
    }

    fn command(
        &mut self,
        words: &[String],
    ) {
        let (command, sub) = match self.parse(words) {
            Ok(parsed) => parsed,
            Err(err) => {
                let _ = err.print();
                if err.use_stderr() {
                    self.last_error = crate::error::EXIT_USAGE;
                }
                return;
            },
        };
        let format = command.own_format().unwrap_or(self.format);
        // stdin carries the shell's commands, never inputs.
        let prepared = match crate::prepare(
            &command,
            format,
            Some(&sub),
            &mut std::io::empty(),
            true,
        ) {
            Ok(prepared) => prepared,
            Err(err) => {
                crate::report_error(&err.response, err.exit, format);
                self.last_error = err.exit;
                return;
            },
        };
        let progress = !self.quiet
            && prepared.input_bytes > progress::PROGRESS_THRESHOLD
            && std::io::stderr().is_terminal();
        let id = crate::request_id();
        {
            let mut own = lock(&self.shared.own);
            own.busy = prepared
                .command
                .is_mutation()
                .then(|| prepared.command.graph().map(str::to_string))
                .flatten();
            own.request = Some(id);
        }
        self.shared.progress.store(progress, Ordering::SeqCst);
        let request = Request::new(prepared.command);
        let response = match &self.mode {
            Mode::Local(session) => session.execute(request),
            Mode::Remote { remote, .. } => {
                let mut response = backend::execute_remote(
                    remote,
                    Request {
                        id: Some(id),
                        ..request
                    },
                );
                response.id = None;
                response
            },
            Mode::Closed => Response::from_result(
                None,
                Err(context_api::ApiError::Internal("no backend".into())),
            ),
        };
        self.shared.progress.store(false, Ordering::SeqCst);
        if progress {
            progress::finish();
        }
        self.record_own(&response);
        let code =
            crate::finish(command.name(), response, &prepared.post, format);
        if code != EXIT_OK {
            self.last_error = code;
        }
    }

    /// Remember the inputs and generations this shell produced.
    fn record_own(
        &self,
        response: &Response,
    ) {
        let mut own = lock(&self.shared.own);
        own.busy = None;
        own.request = None;
        if let Outcome::Ok(CommandResult::Read(report)) = &response.outcome {
            for input in &report.inputs {
                own.inputs.insert((report.graph.clone(), input.id));
            }
            if let Some(saved) = &report.saved {
                own.generations
                    .insert((report.graph.clone(), saved.generation));
            }
        }
    }

    /// Shell settings (`:name args`).
    fn setting(
        &mut self,
        words: &[String],
    ) -> Flow {
        let args: Vec<&str> = words[1..].iter().map(String::as_str).collect();
        match (words[0].as_str(), args.as_slice()) {
            (":quit" | ":q" | ":exit", []) => return Flow::Quit,
            (":help" | ":h", []) => println!("{HELP}"),
            (":format", ["text"]) => self.format = OutputFormat::Text,
            (":format", ["json"]) => self.format = OutputFormat::Json,
            (":server", [url]) => {
                let _ = self.connect(RemoteBackend::new(*url, self.token.clone()));
            },
            (":local", []) => {
                if !matches!(self.mode, Mode::Local(_)) {
                    self.close();
                    self.mode = Mode::Local(local_session(
                        self.store.clone(),
                        self.save,
                        &self.shared,
                    ));
                }
                self.status();
            },
            (":events", [on @ ("on" | "off")]) => {
                self.shared.events.store(*on == "on", Ordering::SeqCst);
                if *on == "on" {
                    match (&self.mode, &self.graph) {
                        (Mode::Local(_) | Mode::Closed, _) => eprintln!(
                            "note: events of other clients exist only on a server (:server URL)"
                        ),
                        (Mode::Remote { .. }, None) =>
                            eprintln!("note: events are shown for the default graph; set one with `use <GRAPH>`"),
                        _ => {},
                    }
                }
            },
            (":save", [policy @ ("on-read" | "manual")]) => {
                let save = if *policy == "manual" {
                    SaveMode::Manual
                } else {
                    SaveMode::OnWrite
                };
                match &self.mode {
                    Mode::Local(_) | Mode::Closed => {
                        self.save = save;
                        // Saves dirty graphs, then reopens with the policy.
                        let closed = self.close();
                        if closed != EXIT_OK {
                            self.last_error = closed;
                        }
                        self.mode = Mode::Local(local_session(
                            self.store.clone(),
                            save,
                            &self.shared,
                        ));
                    },
                    Mode::Remote { .. } => self.fail(CliError::invalid(
                        "the server's --save policy applies in remote mode",
                    )),
                }
            },
            (":status", []) => self.status(),
            (name, _) => self.fail(
                CliError::usage(format!("unknown or incomplete shell command '{}'", words.join(" ")))
                    .with_hint(if SHELL_COMMANDS.contains(&name) {
                        "see :help for the arguments"
                    } else {
                        "shell commands: :format, :server, :local, :events, :save, :status, :help, :quit"
                    }),
            ),
        }
        Flow::Continue
    }

    fn status(&self) {
        let mode = match &self.mode {
            Mode::Local(_) => format!("local (store {})", self.store.display()),
            Mode::Remote { remote, .. } => format!("server {}", remote.url()),
            Mode::Closed => "closed".into(),
        };
        eprintln!(
            "{mode}; graph {}; format {}; events {}",
            self.graph.as_deref().unwrap_or("-"),
            match self.format {
                OutputFormat::Text => "text",
                OutputFormat::Json => "json",
            },
            if self.shared.events.load(Ordering::SeqCst) {
                "on"
            } else {
                "off"
            },
        );
    }

    /// Switch to a server after checking that it answers.
    fn connect(
        &mut self,
        remote: RemoteBackend,
    ) -> Result<(), CliError> {
        if let Err(error) = remote.health() {
            let err = CliError {
                exit: crate::error::exit_code(error.kind),
                response: error,
            };
            self.fail(err.clone());
            return Err(err);
        }
        self.close();
        self.mode = Mode::Remote {
            remote,
            watcher: None,
        };
        self.rewatch();
        Ok(())
    }

    /// (Re)subscribe to the events of the default graph (remote mode).
    fn rewatch(&mut self) {
        let Mode::Remote { remote, watcher } = &mut self.mode else {
            return;
        };
        *watcher = None;
        let Some(graph) = &self.graph else {
            return;
        };
        let shared = self.shared.clone();
        match remote.watch(graph, &[], move |event| shared.on_event(event)) {
            Ok(new) => *watcher = Some(new),
            Err(err) if !self.quiet =>
                eprintln!("warning: no event subscription: {}", err.message),
            Err(_) => {},
        }
    }
}

/// Completion of command names, shell commands and `--flags`.
struct ShellHelper {
    commands: Vec<String>,
    flags: HashMap<String, Vec<String>>,
}

impl ShellHelper {
    fn new() -> Self {
        let line = ShellLine::command();
        let mut commands: Vec<String> =
            SHELL_COMMANDS.iter().map(|c| c.to_string()).collect();
        let mut flags = HashMap::new();
        for sub in line.get_subcommands() {
            commands.push(sub.get_name().to_string());
            flags.insert(
                sub.get_name().to_string(),
                sub.get_arguments()
                    .filter_map(|arg| arg.get_long())
                    .map(|long| format!("--{long}"))
                    .collect(),
            );
        }
        commands.push("help".into());
        commands.sort();
        Self { commands, flags }
    }
}

impl Completer for ShellHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        let before = &line[..pos];
        let start = before
            .rfind(char::is_whitespace)
            .map_or(0, |index| index + 1);
        let word = &before[start..];
        let candidates: Vec<String> =
            match before[..start].split_whitespace().next() {
                None => self.commands.clone(),
                Some(":format") => vec!["text".into(), "json".into()],
                Some(":events") => vec!["on".into(), "off".into()],
                Some(":save") => vec!["on-read".into(), "manual".into()],
                Some(command) if word.starts_with('-') =>
                    self.flags.get(command).cloned().unwrap_or_default(),
                Some(_) => Vec::new(),
            };
        Ok((
            start,
            candidates
                .into_iter()
                .filter(|candidate| candidate.starts_with(word))
                .collect(),
        ))
    }
}

impl Hinter for ShellHelper {
    type Hint = String;
}

impl Highlighter for ShellHelper {}

impl Validator for ShellHelper {}

impl rustyline::Helper for ShellHelper {}
