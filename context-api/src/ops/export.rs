//! Export and import (plan 01 §1.7).

use super::View;
use crate::{
    error::ApiError,
    store::file::GraphMeta,
    types::{
        ExportFormat,
        ExportOutput,
        Gate,
        ImportData,
    },
};
use context_trace::positioned::PositionedGraph;

pub(crate) fn export(
    view: &View<'_>,
    format: ExportFormat,
    gate: &Gate,
    render_cap: Option<usize>,
) -> Result<ExportOutput, ApiError> {
    let _ = (view, format, gate, render_cap);
    Err(ApiError::Unsupported("export is not implemented yet".into()))
}

/// Decode import data into a validated graph and its metadata (input log;
/// `name` is the target graph name).
pub(crate) fn import(
    name: &str,
    data: ImportData,
) -> Result<(GraphMeta, PositionedGraph), ApiError> {
    let _ = (name, data);
    Err(ApiError::Unsupported("import is not implemented yet".into()))
}
