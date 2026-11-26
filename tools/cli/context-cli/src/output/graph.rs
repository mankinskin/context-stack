//! Text renderers of graph lifecycle and query results.

use std::fmt::Write;

use context_api::types::{
    GraphStats,
    GraphSummary,
    InputList,
    ReadReport,
    StepKind,
    ValidationReport,
    VertexInfo,
};

use super::{
    label,
    one_line,
    token,
};

fn headline(summary: &GraphSummary) -> String {
    format!(
        "{}  ({})  {} inputs, {} chars, {} vertices",
        summary.name,
        summary.path,
        summary.input_count,
        summary.total_input_chars,
        summary.vertex_count
    )
}

pub(super) fn summary(
    command: &str,
    summary: &GraphSummary,
) -> String {
    match command {
        "new" => format!("created {}  ({})", summary.name, summary.path),
        "import" => format!("imported {}", headline(summary)),
        _ => {
            let mut out = headline(summary);
            let _ = write!(
                out,
                "\n  atoms {}, composites {}, generation {}\n  created {}, modified {}",
                summary.atom_count,
                summary.composite_count,
                summary.generation,
                summary.created_at,
                summary.modified_at
            );
            if let Some(size) = summary.file_size {
                let _ = write!(out, ", {size} bytes");
            }
            out
        },
    }
}

pub(super) fn list(graphs: &[GraphSummary]) -> String {
    if graphs.is_empty() {
        return "no graphs".into();
    }
    let width = graphs
        .iter()
        .map(|g| g.name.chars().count())
        .max()
        .unwrap_or(0);
    graphs
        .iter()
        .map(|g| {
            format!(
                "{:<width$}  {:>6} inputs  {:>8} vertices  {}",
                g.name, g.input_count, g.vertex_count, g.path
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn read(
    command: &str,
    report: &ReadReport,
) -> String {
    let n = report.inputs.len();
    let inputs = if n == 1 { "input" } else { "inputs" };
    let delta = format!(
        "{} → {} vertices",
        report.vertex_count_before, report.vertex_count_after
    );
    let mut out = if report.dry_run {
        format!(
            "dry run: reading {n} {inputs} into {} would change {delta}; not saved",
            report.graph
        )
    } else if command == "create" {
        format!("created {} from {n} {inputs}: {delta}", report.graph)
    } else {
        format!("read {n} {inputs} into {}: {delta}", report.graph)
    };
    match &report.saved {
        Some(saved) => {
            let _ = write!(
                out,
                "; saved {} ({} bytes, generation {})",
                saved.path, saved.bytes, saved.generation
            );
        },
        None if !report.dry_run => out.push_str("; not saved yet"),
        None => {},
    }
    for input in &report.inputs {
        let _ = write!(
            out,
            "\n  [{}] {}  {} chars → {}",
            input.id,
            one_line(&input.label),
            input.chars,
            token(&input.token)
        );
        for (n, step) in input.steps.iter().flatten().enumerate() {
            let kind = match step.kind {
                StepKind::Unknown => "unknown".to_string(),
                StepKind::Forward { width } => format!("forward {width}"),
                StepKind::Expansion { postfix, expansion } =>
                    format!("expansion postfix {postfix} → {expansion}"),
            };
            let _ = write!(
                out,
                "\n      step {:<4} {kind:<28} root width {:<6} dissolved {} discarded {} stabilized {}",
                n + 1,
                step.root_width,
                step.dissolved,
                step.discarded,
                step.stabilized
            );
        }
    }
    if !report.skipped.is_empty() {
        let skipped: Vec<_> =
            report.skipped.iter().map(|l| one_line(l)).collect();
        let _ = write!(out, "\n  skipped empty: {}", skipped.join(", "));
    }
    out
}

pub(super) fn vertex(info: &VertexInfo) -> String {
    let mut out = format!(
        "{}  width {}{}",
        token(&info.token),
        info.token.width,
        if info.is_atom { "  (atom)" } else { "" }
    );
    if !info.composition.is_empty() {
        out.push_str("\ncomposition:");
        for occ in &info.composition {
            let _ = write!(out, "\n  @{:<5} {}", occ.offset, token(&occ.child));
        }
    }
    if info.parents.is_empty() {
        out.push_str("\nparents: none");
    } else {
        let _ = write!(out, "\nparents ({}):", info.parents.len());
        for parent in &info.parents {
            let _ = write!(
                out,
                "\n  {}  width {}  @{}",
                token(&parent.parent),
                parent.parent.width,
                offsets(&parent.offsets)
            );
        }
    }
    if !info.inputs.is_empty() {
        let ids: Vec<_> =
            info.inputs.iter().map(|id| format!("[{id}]")).collect();
        let _ = write!(out, "\ninput: {}", ids.join(", "));
    }
    out
}

pub(super) fn offsets(offsets: &[usize]) -> String {
    offsets
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn inputs(list: &InputList) -> String {
    let mut out = format!("{} inputs", list.total);
    if list.items.len() < list.total {
        let _ = write!(out, " (showing {})", list.items.len());
    }
    for item in &list.items {
        let _ = write!(
            out,
            "\n  [{}] {}  {} chars → #{}  {}",
            item.id,
            one_line(&item.label),
            item.width,
            item.token,
            item.read_at
        );
    }
    out
}

pub(super) fn stats(stats: &GraphStats) -> String {
    let mut out = headline(&stats.summary);
    let _ = write!(
        out,
        "\n  atoms {}, composites {}, max width {}, avg composition size {:.2}",
        stats.summary.atom_count,
        stats.summary.composite_count,
        stats.max_width,
        stats.avg_composition_size
    );
    out.push_str("\nwidth histogram (width: vertices):");
    for (width, count) in &stats.width_histogram {
        let _ = write!(out, "\n  {width:>6}: {count}");
    }
    out.push_str("\nparent histogram (parent occurrences: vertices):");
    for (parents, count) in &stats.parent_count_histogram {
        let _ = write!(out, "\n  {parents:>6}: {count}");
    }
    out
}

pub(super) fn validation(report: &ValidationReport) -> String {
    if report.valid {
        return "valid".into();
    }
    let mut out = format!(
        "invalid: {} errors, {} retention violations",
        report.errors.len(),
        report.retention_violations.len()
    );
    for error in &report.errors {
        let _ = write!(out, "\n  error: {error}");
    }
    for vertex in &report.retention_violations {
        let _ =
            write!(out, "\n  retention: #{} {}", vertex.index, label(vertex));
    }
    out
}
