//! CLI errors and exit codes.

use context_api::{
    ErrorKind,
    ErrorResponse,
};

/// Exit code of a successful command.
pub const EXIT_OK: u8 = 0;
/// A command error (not found, locked, gate refused, invalid input, …).
pub const EXIT_COMMAND: u8 = 1;
/// A usage error (clap uses 2 as well).
pub const EXIT_USAGE: u8 = 2;
/// An I/O error or a corrupt file.
pub const EXIT_IO: u8 = 3;

/// Exit code for an API error kind.
pub fn exit_code(kind: ErrorKind) -> u8 {
    match kind {
        ErrorKind::Io | ErrorKind::Corrupt => EXIT_IO,
        _ => EXIT_COMMAND,
    }
}

/// An error raised by the CLI itself (input resolution, file output,
/// argument combinations clap cannot express).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub response: ErrorResponse,
    pub exit: u8,
}

impl CliError {
    pub fn new(
        kind: ErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            response: ErrorResponse {
                kind,
                message: message.into(),
                hint: None,
            },
            exit: exit_code(kind),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Io, message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, message)
    }

    /// A usage error detected after parsing (exit 2).
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            exit: EXIT_USAGE,
            ..Self::invalid(message)
        }
    }

    pub fn with_hint(
        mut self,
        hint: impl Into<String>,
    ) -> Self {
        self.response.hint = Some(hint.into());
        self
    }
}
