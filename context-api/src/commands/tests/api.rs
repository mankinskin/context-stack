use crate::{
    error::{
        ApiError,
        ErrorResponse,
        SearchError,
        WorkspaceError,
    },
    types::WorkspaceInfo,
};

use super::{
    super::WorkspaceApi,
    tmp_manager,
};

#[test]
fn workspace_api_trait_is_object_safe_enough() {
    fn use_api(
        api: &mut dyn WorkspaceApi,
        name: &str,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        let info = api.create_workspace(name)?;
        let _ = api.list_workspaces()?;
        let _ = api.save_workspace(name)?;
        Ok(info)
    }

    let (_tmp, mut mgr) = tmp_manager();
    let result = use_api(&mut mgr, "trait-test");
    assert!(result.is_ok());
}

#[test]
fn api_error_to_error_response_round_trip() {
    let api_err = ApiError::Search(SearchError::QueryTooShort);
    let resp = ErrorResponse::from(&api_err);
    assert_eq!(resp.kind, "search");
    assert!(resp.message.contains("too short"));

    let json = serde_json::to_string(&resp).unwrap();
    let deser: ErrorResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.kind, resp.kind);
}
