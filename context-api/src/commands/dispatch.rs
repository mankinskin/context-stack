use crate::{
    error::ApiError,
    tracing_capture::{
        CaptureConfig,
        CaptureResult,
        capture_traced,
    },
    types::TraceSummary,
    workspace::manager::WorkspaceManager,
};

use super::{
    export_import,
    logs,
    Command,
    CommandResult,
};

pub fn execute(
    manager: &mut WorkspaceManager,
    cmd: Command,
) -> Result<CommandResult, ApiError> {
    match cmd {
        Command::CreateWorkspace { name } => {
            let info = manager.create_workspace(&name)?;
            Ok(CommandResult::WorkspaceInfo(info))
        },
        Command::CreateWorkspaceFromNgrams {
            name,
            text,
            timeout_secs,
        } => {
            let info = manager.create_workspace_from_ngrams_text(
                &name,
                &text,
                timeout_secs,
            )?;
            Ok(CommandResult::WorkspaceInfo(info))
        },
        Command::OpenWorkspace { name } => {
            let info = manager.open_workspace(&name)?;
            Ok(CommandResult::WorkspaceInfo(info))
        },
        Command::CloseWorkspace { name } => {
            manager.close_workspace(&name)?;
            Ok(CommandResult::Ok)
        },
        Command::SaveWorkspace { name } => {
            manager.save_workspace(&name)?;
            Ok(CommandResult::Ok)
        },
        Command::ListWorkspaces => {
            let workspaces = manager.list_workspaces()?;
            Ok(CommandResult::WorkspaceInfoList { workspaces })
        },
        Command::DeleteWorkspace { name } => {
            manager.delete_workspace(&name)?;
            Ok(CommandResult::Ok)
        },
        Command::AddAtom { workspace, ch } => {
            let info = manager.add_atom(&workspace, ch)?;
            Ok(CommandResult::AtomInfo(info))
        },
        Command::AddAtoms { workspace, chars } => {
            let atoms = manager.add_atoms(&workspace, chars)?;
            Ok(CommandResult::AtomInfoList { atoms })
        },
        Command::GetAtom { workspace, ch } => {
            let atom = manager.get_atom(&workspace, ch)?;
            Ok(CommandResult::OptionalAtomInfo { atom })
        },
        Command::ListAtoms { workspace } => {
            let atoms = manager.list_atoms(&workspace)?;
            Ok(CommandResult::AtomInfoList { atoms })
        },
        Command::AddSimplePattern { workspace, atoms } => {
            let info = manager.add_simple_pattern(&workspace, atoms)?;
            Ok(CommandResult::PatternInfo(info))
        },
        Command::GetVertex { workspace, index } => {
            let vertex = manager.get_vertex(&workspace, index)?;
            Ok(CommandResult::OptionalVertexInfo { vertex })
        },
        Command::ListVertices { workspace } => {
            let tokens = manager.list_vertices(&workspace)?;
            Ok(CommandResult::TokenInfoList { tokens })
        },
        Command::SearchPattern { workspace, query } => {
            let result = manager.search_pattern(&workspace, query)?;
            Ok(CommandResult::SearchResult(result))
        },
        Command::SearchSequence { workspace, text } => {
            let result = manager.search_sequence(&workspace, &text)?;
            Ok(CommandResult::SearchResult(result))
        },
        Command::InsertFirstMatch { workspace, query } => {
            let result = manager.insert_first_match(&workspace, query)?;
            Ok(CommandResult::InsertResult(result))
        },
        Command::InsertSequence { workspace, text } => {
            let result = manager.insert_sequence(&workspace, &text)?;
            Ok(CommandResult::InsertResult(result))
        },
        Command::InsertSequences { workspace, texts } => {
            let results = manager.insert_sequences(&workspace, texts)?;
            Ok(CommandResult::InsertResultList { results })
        },
        Command::ReadPattern { workspace, index } => {
            let result = manager.read_pattern(&workspace, index)?;
            Ok(CommandResult::ReadResult(result))
        },
        Command::ReadAsText { workspace, index } => {
            let text = manager.read_as_text(&workspace, index)?;
            Ok(CommandResult::Text { text })
        },
        Command::ReadSequence { workspace, text } => {
            let result = manager.read_sequence(&workspace, &text)?;
            Ok(CommandResult::ReadResult(result))
        },
        Command::ReadFile { workspace, path } => {
            let result = manager.read_file(&workspace, &path)?;
            Ok(CommandResult::ReadResult(result))
        },
        Command::GetSnapshot { workspace } => {
            let snapshot = manager.get_snapshot(&workspace)?;
            Ok(CommandResult::Snapshot(snapshot))
        },
        Command::GetStatistics { workspace } => {
            let statistics = manager.get_statistics(&workspace)?;
            Ok(CommandResult::Statistics(statistics))
        },
        Command::ValidateGraph { workspace } => {
            let report = manager.validate_graph(&workspace)?;
            Ok(CommandResult::ValidationReport(report))
        },
        Command::ShowGraph { workspace } => {
            let display = manager.show_graph(&workspace)?;
            Ok(CommandResult::GraphDisplay { display })
        },
        Command::ShowVertex { workspace, index } => {
            let display = manager.show_vertex(&workspace, index)?;
            Ok(CommandResult::GraphDisplay { display })
        },
        Command::RenderAsciiGraph {
            workspace,
            ascii_dag,
        } => {
            let display = manager.render_ascii_graph(&workspace, ascii_dag)?;
            Ok(CommandResult::GraphDisplay { display })
        },
        Command::ListLogs {
            workspace,
            pattern,
            limit,
        } => {
            let log_dir = manager.log_dir(&workspace)?;
            let log_list = logs::list_logs(&log_dir, pattern.as_deref(), limit)
                .map_err(|e| ApiError::Log(e))?;
            Ok(CommandResult::LogList { logs: log_list })
        },
        Command::GetLog {
            workspace,
            filename,
            filter,
            limit,
            offset,
        } => {
            let log_dir = manager.log_dir(&workspace)?;
            let (entries, total) = logs::get_log(
                &log_dir,
                &filename,
                filter.as_deref(),
                limit,
                offset,
            )
            .map_err(|e| ApiError::Log(e))?;
            let returned = entries.len();
            Ok(CommandResult::LogEntries {
                filename,
                total,
                offset,
                limit,
                returned,
                entries,
            })
        },
        Command::QueryLog {
            workspace,
            filename,
            query,
            limit,
        } => {
            let log_dir = manager.log_dir(&workspace)?;
            let (entries, total) =
                logs::query_log(&log_dir, &filename, &query, limit)
                    .map_err(|e| ApiError::Log(e))?;
            Ok(CommandResult::LogQueryResult {
                query,
                matches: total,
                entries,
            })
        },
        Command::AnalyzeLog {
            workspace,
            filename,
        } => {
            let log_dir = manager.log_dir(&workspace)?;
            let analysis = logs::analyze_log(&log_dir, &filename)
                .map_err(|e| ApiError::Log(e))?;
            Ok(CommandResult::LogAnalysis(analysis))
        },
        Command::SearchLogs {
            workspace,
            query,
            limit_per_file,
        } => {
            let log_dir = manager.log_dir(&workspace)?;
            let (results, files_with_matches) =
                logs::search_logs(&log_dir, &query, limit_per_file)
                    .map_err(|e| ApiError::Log(e))?;
            Ok(CommandResult::LogSearchResult {
                query,
                files_with_matches,
                results,
            })
        },
        Command::DeleteLog {
            workspace,
            filename,
        } => {
            let log_dir = manager.log_dir(&workspace)?;
            logs::delete_log(&log_dir, &filename)
                .map_err(|e| ApiError::Log(e))?;
            Ok(CommandResult::Ok)
        },
        Command::DeleteLogs {
            workspace,
            older_than_days,
        } => {
            let log_dir = manager.log_dir(&workspace)?;
            let result = logs::delete_logs(&log_dir, older_than_days)
                .map_err(|e| ApiError::Log(e))?;
            Ok(CommandResult::LogDeleteResult(result))
        },
        Command::ExportWorkspace {
            workspace,
            format,
            path,
        } => {
            let result = export_import::export_workspace(
                manager,
                &workspace,
                format,
                path.as_deref(),
            )?;
            match result {
                Some(data) => Ok(CommandResult::ExportData { data, format }),
                None => Ok(CommandResult::Ok),
            }
        },
        Command::ImportWorkspace {
            name,
            path,
            overwrite,
        } => {
            let info = export_import::import_workspace(
                manager, &name, &path, overwrite,
            )?;
            Ok(CommandResult::WorkspaceInfo(info))
        },
        Command::CompareWorkspaces {
            workspace_a,
            workspace_b,
            mode,
        } => {
            let result = manager
                .compare_workspaces(&workspace_a, &workspace_b, mode)
                .map_err(ApiError::Compare)?;
            Ok(CommandResult::GraphDiff(result))
        },
        Command::CompareVertices {
            workspace_a,
            index_a,
            workspace_b,
            index_b,
        } => {
            let result = manager
                .compare_vertices(&workspace_a, index_a, &workspace_b, index_b)
                .map_err(ApiError::Compare)?;
            Ok(CommandResult::GraphDiff(result))
        },
    }
}

pub fn execute_traced(
    manager: &mut WorkspaceManager,
    cmd: Command,
    config: Option<&CaptureConfig>,
) -> Result<(CommandResult, Option<TraceSummary>), ApiError> {
    match config {
        Some(cfg) if cfg.enabled => {
            let command_name = cmd.command_name();
            let cmd_clone = cmd;
            let capture: CaptureResult<Result<CommandResult, ApiError>> =
                capture_traced(cfg, command_name, || {
                    execute(manager, cmd_clone)
                });
            let result = capture.result?;
            Ok((result, capture.summary))
        },
        _ => {
            let result = execute(manager, cmd)?;
            Ok((result, None))
        },
    }
}