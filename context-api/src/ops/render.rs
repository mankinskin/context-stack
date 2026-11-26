//! Render (ascii, dot, mermaid, json) with the size gate (plan 01 §1.6).

use super::View;
use crate::{
    error::ApiError,
    types::{
        Gate,
        Limits,
        RenderFormat,
        RenderOutput,
        VertexRef,
    },
};

/// `render_cap`: the session's hard vertex cap that `gate.force` cannot lift.
pub(crate) fn render(
    view: &View<'_>,
    format: RenderFormat,
    root: Option<&VertexRef>,
    depth: Option<usize>,
    gate: &Gate,
    limits: &Limits,
    render_cap: Option<usize>,
) -> Result<RenderOutput, ApiError> {
    let _ = (view, format, root, depth, gate, limits, render_cap);
    Err(ApiError::Unsupported("render is not implemented yet".into()))
}
