//! HTTP error response mapping.
//!
//! Maps `ApiError` variants from `context-api` to appropriate HTTP status
//! codes and JSON error response bodies.

use context_api::error::{
    ApiError,
    AtomError,
    InsertError,
    LogError,
    PatternError,
    ReadError,
    SearchError,
    WorkspaceError,
};
use viewer_api::axum::{
    http::StatusCode,
    response::{
        IntoResponse,
        Response,
    },
    Json,
};

/// JSON error response body returned to HTTP clients.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HttpErrorBody {
    /// Human-readable error message.
    pub error: String,
    /// Error category tag (e.g. "workspace", "atom", "pattern", "internal").
    pub kind: String,
}

/// Unified HTTP error type used by all handlers.
///
/// Wraps either an `ApiError` from the domain layer or an internal server
/// error (e.g. a poisoned mutex or a `JoinError` from `spawn_blocking`).
pub enum HttpError {
    /// A domain-level error from `context-api`.
    Api(ApiError),
    /// An internal server error not originating from the domain layer.
    Internal(String),
    /// A bad-request error (e.g. malformed or unrecognised JSON body).
    BadRequest(String),
}

impl HttpError {
    /// Create an internal server error with a descriptive message.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Create a bad-request error (e.g. unparseable JSON body).
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
}

impl From<ApiError> for HttpError {
    fn from(err: ApiError) -> Self {
        Self::Api(err)
    }
}

/// Map an `ApiError` to an HTTP status code.
fn status_for_api_error(err: &ApiError) -> StatusCode {
    match err {
        ApiError::Workspace(e) => match e {
            WorkspaceError::NotFound { .. } => StatusCode::NOT_FOUND,
            WorkspaceError::AlreadyExists { .. } => StatusCode::CONFLICT,
            WorkspaceError::NotOpen { .. } => StatusCode::BAD_REQUEST,
            WorkspaceError::AlreadyOpen { .. } => StatusCode::CONFLICT,
            WorkspaceError::LockConflict { .. } => StatusCode::LOCKED,
            WorkspaceError::IoError(_)
            | WorkspaceError::SerializationError(_) =>
                StatusCode::INTERNAL_SERVER_ERROR,
            WorkspaceError::NgramsTimeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            WorkspaceError::NgramsFailed { .. } =>
                StatusCode::INTERNAL_SERVER_ERROR,
        },
        ApiError::Atom(e) => match e {
            AtomError::WorkspaceNotOpen { .. } => StatusCode::BAD_REQUEST,
        },
        ApiError::Pattern(e) => match e {
            PatternError::WorkspaceNotOpen { .. } => StatusCode::BAD_REQUEST,
            PatternError::AtomNotFound { .. } => StatusCode::NOT_FOUND,
            PatternError::VertexNotFound { .. } => StatusCode::NOT_FOUND,
            PatternError::TooShort { .. }
            | PatternError::DuplicateAtomInInput { .. } =>
                StatusCode::UNPROCESSABLE_ENTITY,
            PatternError::AtomAlreadyInPattern { .. } => StatusCode::CONFLICT,
            PatternError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        ApiError::Search(e) => match e {
            SearchError::WorkspaceNotOpen { .. } => StatusCode::BAD_REQUEST,
            SearchError::TokenNotFound { .. } => StatusCode::NOT_FOUND,
            SearchError::QueryTooShort => StatusCode::UNPROCESSABLE_ENTITY,
            SearchError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        ApiError::Insert(e) => match e {
            InsertError::WorkspaceNotOpen { .. } => StatusCode::BAD_REQUEST,
            InsertError::TokenNotFound { .. } => StatusCode::NOT_FOUND,
            InsertError::QueryTooShort => StatusCode::UNPROCESSABLE_ENTITY,
            InsertError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        ApiError::Read(e) => match e {
            ReadError::WorkspaceNotOpen { .. } => StatusCode::BAD_REQUEST,
            ReadError::VertexNotFound { .. } => StatusCode::NOT_FOUND,
            ReadError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ReadError::SequenceTooShort { .. } =>
                StatusCode::UNPROCESSABLE_ENTITY,
            ReadError::FileReadError { .. } => StatusCode::BAD_REQUEST,
        },
        ApiError::Log(e) => match e {
            LogError::FileNotFound { .. } => StatusCode::NOT_FOUND,
            LogError::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            LogError::QueryError(_) => StatusCode::BAD_REQUEST,
            LogError::DirectoryError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        ApiError::Compare(e) => match e {
            context_api::error::CompareError::WorkspaceNotOpen { .. } =>
                StatusCode::BAD_REQUEST,
            context_api::error::CompareError::VertexNotFound { .. } =>
                StatusCode::NOT_FOUND,
        },
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        match self {
            HttpError::Api(err) => {
                let status = status_for_api_error(&err);
                let body = HttpErrorBody {
                    error: err.to_string(),
                    kind: err.kind().to_string(),
                };
                (status, Json(body)).into_response()
            },
            HttpError::Internal(msg) => {
                let body = HttpErrorBody {
                    error: msg,
                    kind: "internal".to_string(),
                };
                (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
            },
            HttpError::BadRequest(msg) => {
                let body = HttpErrorBody {
                    error: msg,
                    kind: "bad_request".to_string(),
                };
                (StatusCode::BAD_REQUEST, Json(body)).into_response()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_not_found_maps_to_404() {
        let err = ApiError::Workspace(WorkspaceError::NotFound {
            name: "test".to_string(),
        });
        assert_eq!(status_for_api_error(&err), StatusCode::NOT_FOUND);
    }

    #[test]
    fn workspace_already_exists_maps_to_409() {
        let err = ApiError::Workspace(WorkspaceError::AlreadyExists {
            name: "test".to_string(),
        });
        assert_eq!(status_for_api_error(&err), StatusCode::CONFLICT);
    }

    #[test]
    fn workspace_not_open_maps_to_400() {
        let err = ApiError::Workspace(WorkspaceError::NotOpen {
            name: "test".to_string(),
        });
        assert_eq!(status_for_api_error(&err), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn workspace_already_open_maps_to_409() {
        let err = ApiError::Workspace(WorkspaceError::AlreadyOpen {
            name: "test".to_string(),
        });
        assert_eq!(status_for_api_error(&err), StatusCode::CONFLICT);
    }

    #[test]
    fn workspace_lock_conflict_maps_to_423() {
        let err = ApiError::Workspace(WorkspaceError::LockConflict {
            name: "test".to_string(),
        });
        assert_eq!(status_for_api_error(&err), StatusCode::LOCKED);
    }

    #[test]
    fn atom_workspace_not_open_maps_to_400() {
        let err = ApiError::Atom(AtomError::WorkspaceNotOpen {
            workspace: "test".to_string(),
        });
        assert_eq!(status_for_api_error(&err), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn pattern_atom_not_found_maps_to_404() {
        let err = ApiError::Pattern(PatternError::AtomNotFound { ch: 'x' });
        assert_eq!(status_for_api_error(&err), StatusCode::NOT_FOUND);
    }

    #[test]
    fn pattern_too_short_maps_to_422() {
        let err = ApiError::Pattern(PatternError::TooShort { len: 1 });
        assert_eq!(
            status_for_api_error(&err),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[test]
    fn search_query_too_short_maps_to_422() {
        let err = ApiError::Search(SearchError::QueryTooShort);
        assert_eq!(
            status_for_api_error(&err),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[test]
    fn search_internal_error_maps_to_500() {
        let err =
            ApiError::Search(SearchError::InternalError("oops".to_string()));
        assert_eq!(
            status_for_api_error(&err),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn insert_token_not_found_maps_to_404() {
        let err = ApiError::Insert(InsertError::TokenNotFound {
            description: "missing".to_string(),
        });
        assert_eq!(status_for_api_error(&err), StatusCode::NOT_FOUND);
    }

    #[test]
    fn read_vertex_not_found_maps_to_404() {
        let err = ApiError::Read(ReadError::VertexNotFound { index: 42 });
        assert_eq!(status_for_api_error(&err), StatusCode::NOT_FOUND);
    }

    #[test]
    fn log_file_not_found_maps_to_404() {
        let err = ApiError::Log(LogError::FileNotFound {
            filename: "test.log".to_string(),
        });
        assert_eq!(status_for_api_error(&err), StatusCode::NOT_FOUND);
    }

    #[test]
    fn log_query_error_maps_to_400() {
        let err = ApiError::Log(LogError::QueryError("bad query".to_string()));
        assert_eq!(status_for_api_error(&err), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn internal_error_constructor() {
        let err = HttpError::internal("something broke");
        match err {
            HttpError::Internal(msg) => assert_eq!(msg, "something broke"),
            _ => panic!("expected Internal variant"),
        }
    }

    #[test]
    fn bad_request_error_constructor() {
        let err = HttpError::bad_request("invalid json");
        match err {
            HttpError::BadRequest(msg) => assert_eq!(msg, "invalid json"),
            _ => panic!("expected BadRequest variant"),
        }
    }

    #[test]
    fn api_error_converts_via_from() {
        let api_err = ApiError::Workspace(WorkspaceError::NotFound {
            name: "ws".to_string(),
        });
        let http_err: HttpError = api_err.into();
        match http_err {
            HttpError::Api(_) => {}, // expected
            _ => panic!("expected Api variant"),
        }
    }

    #[test]
    fn http_error_body_serializes_correctly() {
        let body = HttpErrorBody {
            error: "not found".to_string(),
            kind: "workspace".to_string(),
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["error"], "not found");
        assert_eq!(json["kind"], "workspace");
    }
}
