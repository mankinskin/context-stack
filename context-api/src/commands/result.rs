use serde::{
    Deserialize,
    Serialize,
};

use context_trace::graph::snapshot::GraphSnapshot;

use crate::{
    ts_export,
    types::{
        AtomInfo,
        GraphDiffResult,
        GraphStatistics,
        InsertResult,
        LogAnalysis,
        LogDeleteResult,
        LogEntryInfo,
        LogFileInfo,
        LogFileSearchResult,
        PatternInfo,
        PatternReadResult,
        SearchResult,
        TokenInfo,
        ValidationReport,
        VertexInfo,
        WorkspaceInfo,
    },
};

use super::export_import::ExportFormat;

ts_export! {
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum CommandResult {
        WorkspaceInfo(WorkspaceInfo),
        WorkspaceInfoList { workspaces: Vec<WorkspaceInfo> },
        AtomInfo(AtomInfo),
        AtomInfoList { atoms: Vec<AtomInfo> },
        OptionalAtomInfo { atom: Option<AtomInfo> },
        PatternInfo(PatternInfo),
        OptionalVertexInfo { vertex: Option<VertexInfo> },
        TokenInfoList { tokens: Vec<TokenInfo> },
        SearchResult(SearchResult),
        InsertResult(InsertResult),
        InsertResultList { results: Vec<InsertResult> },
        ReadResult(PatternReadResult),
        Text { text: String },
        Snapshot(#[schemars(with = "serde_json::Value")] GraphSnapshot),
        GraphDiff(#[schemars(with = "serde_json::Value")] GraphDiffResult),
        Statistics(GraphStatistics),
        ValidationReport(ValidationReport),
        GraphDisplay { display: String },
        Ok,
        LogList { logs: Vec<LogFileInfo> },
        LogEntries {
            filename: String,
            total: usize,
            offset: usize,
            limit: usize,
            returned: usize,
            entries: Vec<LogEntryInfo>,
        },
        LogQueryResult {
            query: String,
            matches: usize,
            entries: Vec<LogEntryInfo>,
        },
        LogAnalysis(LogAnalysis),
        LogSearchResult {
            query: String,
            files_with_matches: usize,
            results: Vec<LogFileSearchResult>,
        },
        LogDeleteResult(LogDeleteResult),
        ExportData {
            #[schemars(with = "String")]
            #[cfg_attr(feature = "ts-gen", ts(type = "number[]"))]
            data: Vec<u8>,
            format: ExportFormat,
        },
    }
}
