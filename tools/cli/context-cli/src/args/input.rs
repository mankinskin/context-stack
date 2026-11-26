//! INPUT arguments of `read` and `create`.

use std::path::PathBuf;

use clap::{
    ArgMatches,
    Args,
    ValueEnum,
};
use context_api::types::Split;

use crate::input::{
    InputOptions,
    Source,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum SplitArg {
    /// Each source is one input.
    #[default]
    None,
    /// One input per non-empty line.
    Lines,
    /// One input per blank-line separated paragraph.
    Paragraphs,
}

impl From<SplitArg> for Split {
    fn from(split: SplitArg) -> Self {
        match split {
            SplitArg::None => Split::None,
            SplitArg::Lines => Split::Lines,
            SplitArg::Paragraphs => Split::Paragraphs,
        }
    }
}

#[derive(Debug, Args)]
pub struct InputArgs {
    /// Text inputs, kept in order; `-` reads stdin (also read when no INPUT
    /// is given and stdin is not a terminal).
    #[arg(value_name = "INPUT")]
    pub texts: Vec<String>,
    /// Explicit text input (for text starting with '-').
    #[arg(
        short = 't',
        long = "text",
        value_name = "TEXT",
        allow_hyphen_values = true
    )]
    pub text: Vec<String>,
    /// A file input; a directory with --recursive.
    #[arg(short = 'f', long = "file", value_name = "PATH")]
    pub file: Vec<PathBuf>,
    /// Read the files below directories given with -f.
    #[arg(short = 'r', long)]
    pub recursive: bool,
    /// Only files matching GLOB (relative to the directory) with --recursive.
    #[arg(long, value_name = "GLOB", requires = "recursive")]
    pub glob: Option<String>,
    /// Split every source into several inputs.
    #[arg(long, value_enum, default_value = "none")]
    pub split: SplitArg,
}

impl InputArgs {
    /// The sources in command-line order. `matches` are the matches of the
    /// subcommand; without them the order is texts, -t, -f.
    pub fn sources(
        &self,
        matches: Option<&ArgMatches>,
    ) -> Vec<Source> {
        let mut fallback = 0;
        let mut indices = |id: &str, len: usize| -> Vec<usize> {
            matches
                .and_then(|m| m.indices_of(id))
                .map(|i| i.collect::<Vec<_>>())
                .filter(|i| i.len() == len)
                .unwrap_or_else(|| {
                    fallback += len;
                    (fallback - len..fallback).collect()
                })
        };
        let mut sources: Vec<(usize, Source)> = Vec::new();
        for (text, index) in
            self.texts.iter().zip(indices("texts", self.texts.len()))
        {
            let source = if text == "-" {
                Source::Stdin
            } else {
                Source::Text(text.clone())
            };
            sources.push((index, source));
        }
        for (text, index) in
            self.text.iter().zip(indices("text", self.text.len()))
        {
            sources.push((index, Source::Text(text.clone())));
        }
        for (file, index) in
            self.file.iter().zip(indices("file", self.file.len()))
        {
            sources.push((index, Source::File(file.clone())));
        }
        sources.sort_by_key(|(index, _)| *index);
        sources.into_iter().map(|(_, source)| source).collect()
    }

    pub fn options(&self) -> InputOptions {
        InputOptions {
            recursive: self.recursive,
            glob: self.glob.clone(),
        }
    }
}
