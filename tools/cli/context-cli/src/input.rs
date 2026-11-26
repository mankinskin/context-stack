//! INPUT resolution: text arguments, files, directories and stdin become
//! `Input::Text` values (files are read here, on the client side, with the
//! path as label), in command-line order.

use std::{
    fs,
    io::Read,
    path::{
        Path,
        PathBuf,
    },
};

use context_api::types::Input;
use globset::{
    Glob,
    GlobMatcher,
};

use crate::error::CliError;

/// Label of stdin inputs.
pub const STDIN_LABEL: &str = "<stdin>";

/// One INPUT source from the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Text(String),
    File(PathBuf),
    Stdin,
}

/// Options for file and directory sources.
#[derive(Debug, Clone, Default)]
pub struct InputOptions {
    /// Walk directories given with `-f`.
    pub recursive: bool,
    /// Only files whose path relative to the walked directory matches.
    pub glob: Option<String>,
}

/// Add implicit stdin: no INPUT given and stdin is not a terminal.
pub fn with_implicit_stdin(
    mut sources: Vec<Source>,
    stdin_is_terminal: bool,
) -> Vec<Source> {
    if sources.is_empty() && !stdin_is_terminal {
        sources.push(Source::Stdin);
    }
    sources
}

/// Resolve sources to reader inputs, preserving their order.
pub fn resolve(
    sources: &[Source],
    options: &InputOptions,
    stdin: &mut dyn Read,
) -> Result<Vec<Input>, CliError> {
    let matcher = options
        .glob
        .as_deref()
        .map(|pattern| {
            Glob::new(pattern)
                .map(|glob| glob.compile_matcher())
                .map_err(|e| {
                    CliError::usage(format!("invalid --glob '{pattern}': {e}"))
                })
        })
        .transpose()?;
    let mut stdin_used = false;
    let mut inputs = Vec::new();
    for source in sources {
        match source {
            Source::Text(text) => inputs.push(Input::Text {
                text: text.clone(),
                label: None,
            }),
            Source::Stdin => {
                if stdin_used {
                    return Err(CliError::usage(
                        "stdin ('-') given more than once",
                    ));
                }
                stdin_used = true;
                let mut bytes = Vec::new();
                stdin.read_to_end(&mut bytes).map_err(|e| {
                    CliError::io(format!("cannot read {STDIN_LABEL}: {e}"))
                })?;
                inputs.push(text_input(STDIN_LABEL, bytes)?);
            },
            Source::File(path) => {
                let meta = fs::metadata(path).map_err(|e| {
                    CliError::io(format!(
                        "cannot read '{}': {e}",
                        path.display()
                    ))
                })?;
                if meta.is_dir() {
                    if !options.recursive {
                        return Err(CliError::invalid(format!(
                            "'{}' is a directory",
                            path.display()
                        ))
                        .with_hint("pass --recursive [--glob '*.txt'] to read the files below it"));
                    }
                    let mut files = Vec::new();
                    walk(path, path, matcher.as_ref(), &mut files)?;
                    for file in files {
                        inputs.push(read_file(&file)?);
                    }
                } else {
                    inputs.push(read_file(path)?);
                }
            },
        }
    }
    Ok(inputs)
}

/// Total size of the input texts in bytes.
pub fn total_bytes(inputs: &[Input]) -> usize {
    inputs
        .iter()
        .map(|input| match input {
            Input::Text { text, .. } => text.len(),
            Input::File { .. } => 0,
        })
        .sum()
}

fn read_file(path: &Path) -> Result<Input, CliError> {
    let bytes = fs::read(path).map_err(|e| {
        CliError::io(format!("cannot read '{}': {e}", path.display()))
    })?;
    text_input(&path.display().to_string(), bytes)
}

fn text_input(
    label: &str,
    bytes: Vec<u8>,
) -> Result<Input, CliError> {
    let text = String::from_utf8(bytes)
        .map_err(|_| CliError::io(format!("'{label}' is not valid UTF-8")))?;
    Ok(Input::Text {
        text,
        label: Some(label.to_string()),
    })
}

/// Files below `dir` in sorted order (entries sorted by name, depth first);
/// symlinked directories are not followed.
fn walk(
    root: &Path,
    dir: &Path,
    matcher: Option<&GlobMatcher>,
    files: &mut Vec<PathBuf>,
) -> Result<(), CliError> {
    let io_err = |e: std::io::Error| {
        CliError::io(format!("cannot read '{}': {e}", dir.display()))
    };
    let mut entries = fs::read_dir(dir)
        .map_err(io_err)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_err)?;
    entries.sort();
    for path in entries {
        let link = fs::symlink_metadata(&path).map_err(io_err)?;
        if link.is_dir() {
            walk(root, &path, matcher, files)?;
        } else if path.is_file() {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            if matcher.is_none_or(|m| m.is_match(relative)) {
                files.push(path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(input: &Input) -> (&str, Option<&str>) {
        match input {
            Input::Text { text, label } => (text, label.as_deref()),
            Input::File { .. } => panic!("file input"),
        }
    }

    #[test]
    fn order_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "from file").unwrap();
        let sources = [
            Source::Text("first".into()),
            Source::Stdin,
            Source::File(file.clone()),
            Source::Text("last".into()),
        ];
        let inputs = resolve(
            &sources,
            &InputOptions::default(),
            &mut "from stdin".as_bytes(),
        )
        .unwrap();
        let got: Vec<_> = inputs.iter().map(text).collect();
        let path = file.display().to_string();
        assert_eq!(
            got,
            [
                ("first", None),
                ("from stdin", Some(STDIN_LABEL)),
                ("from file", Some(path.as_str())),
                ("last", None),
            ]
        );
    }

    #[test]
    fn implicit_stdin_only_without_sources_and_terminal() {
        assert_eq!(with_implicit_stdin(vec![], false), [Source::Stdin]);
        assert!(with_implicit_stdin(vec![], true).is_empty());
        let text = vec![Source::Text("x".into())];
        assert_eq!(with_implicit_stdin(text.clone(), false), text);
    }

    #[test]
    fn stdin_twice_is_refused() {
        let err = resolve(
            &[Source::Stdin, Source::Stdin],
            &InputOptions::default(),
            &mut "x".as_bytes(),
        )
        .unwrap_err();
        assert_eq!(err.exit, crate::error::EXIT_USAGE);
    }

    #[test]
    fn directory_walk_is_sorted_and_filtered() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("b/sub")).unwrap();
        fs::write(root.join("c.md"), "c").unwrap();
        fs::write(root.join("a.md"), "a").unwrap();
        fs::write(root.join("a.txt"), "skip").unwrap();
        fs::write(root.join("b/sub/d.md"), "d").unwrap();
        fs::write(root.join("b/e.md"), "e").unwrap();
        let options = InputOptions {
            recursive: true,
            glob: Some("*.md".into()),
        };
        let inputs = resolve(
            &[Source::File(root.into())],
            &options,
            &mut std::io::empty(),
        )
        .unwrap();
        let texts: Vec<_> = inputs.iter().map(|i| text(i).0).collect();
        assert_eq!(texts, ["a", "e", "d", "c"]);
        let label = text(&inputs[2]).1.unwrap().to_string();
        assert!(label.ends_with("d.md") && label.contains("sub"), "{label}");
    }

    #[test]
    fn directory_needs_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve(
            &[Source::File(dir.path().into())],
            &InputOptions::default(),
            &mut std::io::empty(),
        )
        .unwrap_err();
        assert!(err.response.hint.unwrap().contains("--recursive"));
    }

    #[test]
    fn invalid_utf8_names_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bad.bin");
        fs::write(&file, [0x61, 0xff, 0xfe]).unwrap();
        let err = resolve(
            &[Source::File(file)],
            &InputOptions::default(),
            &mut std::io::empty(),
        )
        .unwrap_err();
        assert_eq!(err.exit, crate::error::EXIT_IO);
        assert!(
            err.response.message.contains("bad.bin"),
            "{}",
            err.response.message
        );
        let err = resolve(
            &[Source::Stdin],
            &InputOptions::default(),
            &mut [0xffu8].as_slice(),
        )
        .unwrap_err();
        assert!(err.response.message.contains(STDIN_LABEL));
    }
}
