//! HTTP routes: `/v1/execute`, the REST convenience routes, auth, CORS,
//! body limit and the static frontend.

use std::{
    collections::HashMap,
    path::PathBuf,
};

use axum::{
    body::{
        Body,
        Bytes,
    },
    extract::{
        rejection::BytesRejection,
        DefaultBodyLimit,
        Path,
        Query,
        Request as HttpRequest,
        State,
    },
    http::{
        header,
        HeaderValue,
        Method,
        StatusCode,
    },
    middleware::{
        self,
        Next,
    },
    response::{
        IntoResponse,
        Response as HttpResponse,
    },
    routing::{
        get,
        post,
    },
    Json,
    Router,
};
use context_api::{
    types::{
        Gate,
        Input,
        Limits,
        RenderFormat,
        SavePolicy,
        SearchView,
        Split,
        VertexRef,
    },
    Command,
    CommandResult,
    ErrorKind,
    ErrorResponse,
    Outcome,
    Request,
    Response,
};
use serde::de::DeserializeOwned;
use tower_http::{
    cors::{
        AllowOrigin,
        CorsLayer,
    },
    services::{
        ServeDir,
        ServeFile,
    },
};

use crate::{
    state::AppState,
    ws,
};

/// HTTP status of an error kind.
pub fn status_for(kind: ErrorKind) -> StatusCode {
    match kind {
        ErrorKind::NotFound => StatusCode::NOT_FOUND,
        ErrorKind::AlreadyExists | ErrorKind::Locked => StatusCode::CONFLICT,
        ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
        ErrorKind::Forbidden => StatusCode::FORBIDDEN,
        ErrorKind::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        ErrorKind::Unsupported => StatusCode::NOT_IMPLEMENTED,
        ErrorKind::Corrupt | ErrorKind::Io | ErrorKind::Internal =>
            StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// The router of a server.
pub fn router(state: AppState) -> Router {
    let config = state.config().clone();
    let api = Router::new()
        .route("/v1/execute", post(execute))
        .route("/v1/ws", get(ws::handler))
        .route("/v1/graphs", get(list))
        .route("/v1/graphs/{graph}", get(info))
        .route("/v1/graphs/{graph}/search", get(search))
        .route("/v1/graphs/{graph}/vertex/{vertex}", get(vertex))
        .route("/v1/graphs/{graph}/render", get(render))
        .route("/v1/graphs/{graph}/read", post(read))
        .route("/v1/graphs/{graph}/save", post(save))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .route("/v1/health", get(health))
        .layer(DefaultBodyLimit::max(config.max_body))
        .with_state(state);
    let app = with_static_files(api, config.static_dir);
    if config.cors_origins.is_empty() {
        return app;
    }
    let origins: Vec<HeaderValue> = config
        .cors_origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect();
    app.layer(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
    )
}

fn with_static_files(
    router: Router,
    static_dir: Option<PathBuf>,
) -> Router {
    match static_dir {
        Some(dir) if dir.is_dir() => {
            let index = ServeFile::new(dir.join("index.html"));
            router.fallback_service(ServeDir::new(dir).fallback(index))
        },
        _ => router,
    }
}

// ── responses ───────────────────────────────────────────────────────

/// The response JSON with the status of its outcome.
pub(crate) fn reply(response: Response) -> HttpResponse {
    let status = match &response.outcome {
        Outcome::Ok(_) => StatusCode::OK,
        Outcome::Error(error) => status_for(error.kind),
    };
    (status, Json(response)).into_response()
}

pub(crate) fn error(
    kind: ErrorKind,
    message: impl Into<String>,
) -> ErrorResponse {
    ErrorResponse {
        kind,
        message: message.into(),
        hint: None,
    }
}

fn error_reply(
    status: StatusCode,
    error: ErrorResponse,
) -> HttpResponse {
    let response = Response {
        id: None,
        outcome: Outcome::Error(error),
    };
    (status, Json(response)).into_response()
}

/// A refused request, replied as `{"error": ...}` with its status.
struct Rejected(StatusCode, ErrorResponse);

impl IntoResponse for Rejected {
    fn into_response(self) -> HttpResponse {
        error_reply(self.0, self.1)
    }
}

fn bad_request(message: impl Into<String>) -> Rejected {
    Rejected(
        StatusCode::BAD_REQUEST,
        error(ErrorKind::InvalidInput, message),
    )
}

/// The request body, or the error reply of a failed read (413 above the
/// body limit).
fn body(body: Result<Bytes, BytesRejection>) -> Result<Bytes, Rejected> {
    body.map_err(|rejection| {
        let status = rejection.status();
        let kind = if status == StatusCode::PAYLOAD_TOO_LARGE {
            ErrorKind::TooLarge
        } else {
            ErrorKind::InvalidInput
        };
        Rejected(status, error(kind, rejection.body_text()))
    })
}

// ── auth ────────────────────────────────────────────────────────────

fn constant_time_eq(
    a: &[u8],
    b: &[u8],
) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Require the configured bearer token (`Authorization: Bearer <token>`,
/// or `?token=` on the WebSocket route).
async fn auth(
    State(state): State<AppState>,
    request: HttpRequest,
    next: Next,
) -> HttpResponse {
    let Some(token) = state.config().token.as_deref() else {
        return next.run(request).await;
    };
    let matches = |candidate: &str| {
        constant_time_eq(candidate.as_bytes(), token.as_bytes())
    };
    let by_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|candidate| matches(candidate.trim()));
    let by_query = request.uri().path() == "/v1/ws"
        && Query::<HashMap<String, String>>::try_from_uri(request.uri())
            .ok()
            .and_then(|query| {
                query.0.get("token").map(|candidate| matches(candidate))
            })
            .unwrap_or(false);
    if by_header || by_query {
        return next.run(request).await;
    }
    let mut response = error_reply(
        StatusCode::UNAUTHORIZED,
        error(ErrorKind::Forbidden, "missing or invalid bearer token"),
    );
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

// ── handlers ────────────────────────────────────────────────────────

async fn health(State(state): State<AppState>) -> HttpResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "graphs_open": state.open_graphs().await,
    }))
    .into_response()
}

async fn execute(
    State(state): State<AppState>,
    raw: Result<Bytes, BytesRejection>,
) -> HttpResponse {
    let raw = match body(raw) {
        Ok(raw) => raw,
        Err(rejected) => return rejected.into_response(),
    };
    match serde_json::from_slice::<Request>(&raw) {
        Ok(request) => reply(state.execute(request).await),
        Err(err) =>
            bad_request(format!("invalid request: {err}")).into_response(),
    }
}

async fn run(
    state: &AppState,
    command: Command,
) -> HttpResponse {
    reply(state.execute(Request::new(command)).await)
}

async fn list(State(state): State<AppState>) -> HttpResponse {
    run(&state, Command::List).await
}

async fn info(
    State(state): State<AppState>,
    Path(graph): Path<String>,
) -> HttpResponse {
    run(&state, Command::Info { graph }).await
}

async fn save(
    State(state): State<AppState>,
    Path(graph): Path<String>,
) -> HttpResponse {
    run(&state, Command::Save { graph }).await
}

type Params = Query<HashMap<String, String>>;

/// A `snake_case` enum value of a query parameter.
fn enum_param<T: DeserializeOwned>(
    params: &HashMap<String, String>,
    name: &str,
) -> Result<Option<T>, Rejected> {
    params
        .get(name)
        .map(|value| {
            serde_json::from_value(serde_json::Value::String(value.clone()))
                .map_err(|_| bad_request(format!("invalid {name} '{value}'")))
        })
        .transpose()
}

fn usize_param(
    params: &HashMap<String, String>,
    name: &str,
) -> Result<Option<usize>, Rejected> {
    params
        .get(name)
        .map(|value| {
            value.parse().map_err(|_| {
                bad_request(format!(
                    "invalid {name} '{value}': expected a number"
                ))
            })
        })
        .transpose()
}

fn bool_param(
    params: &HashMap<String, String>,
    name: &str,
) -> Result<bool, Rejected> {
    match params.get(name).map(String::as_str) {
        None | Some("false" | "0" | "no") => Ok(false),
        Some("" | "true" | "1" | "yes") => Ok(true),
        Some(value) => Err(bad_request(format!(
            "invalid {name} '{value}': expected true or false"
        ))),
    }
}

/// Digits select a vertex by index, anything else by text.
fn vertex_ref(text: String) -> VertexRef {
    match text.parse() {
        Ok(index) if text.bytes().all(|b| b.is_ascii_digit()) =>
            VertexRef::Index(index),
        _ => VertexRef::Text(text),
    }
}

async fn search(
    State(state): State<AppState>,
    Path(graph): Path<String>,
    Query(params): Params,
) -> HttpResponse {
    let command: Result<Command, Rejected> = (|| {
        let query = params
            .get("q")
            .cloned()
            .ok_or_else(|| bad_request("missing query parameter q"))?;
        let view: Option<SearchView> = enum_param(&params, "view")?;
        let limits = Limits {
            limit: usize_param(&params, "limit")?,
            context_width: usize_param(&params, "width")?,
            ..Limits::default()
        };
        Ok(Command::Search {
            graph,
            query,
            view: view.unwrap_or_default(),
            limits,
        })
    })();
    match command {
        Ok(command) => run(&state, command).await,
        Err(rejected) => rejected.into_response(),
    }
}

async fn vertex(
    State(state): State<AppState>,
    Path((graph, vertex)): Path<(String, String)>,
) -> HttpResponse {
    run(
        &state,
        Command::Vertex {
            graph,
            vertex: vertex_ref(vertex),
            limits: Limits::default(),
        },
    )
    .await
}

async fn render(
    State(state): State<AppState>,
    Path(graph): Path<String>,
    Query(params): Params,
) -> HttpResponse {
    let command: Result<Command, Rejected> = (|| {
        let format: Option<RenderFormat> = enum_param(&params, "format")?;
        Ok(Command::Render {
            graph,
            format: format.unwrap_or_default(),
            root: params.get("root").cloned().map(vertex_ref),
            depth: usize_param(&params, "depth")?,
            gate: Gate {
                force: bool_param(&params, "force")?,
                ..Gate::default()
            },
            limits: Limits::default(),
        })
    })();
    let command = match command {
        Ok(command) => command,
        Err(rejected) => return rejected.into_response(),
    };
    let response = state.execute(Request::new(command)).await;
    let content_type = match &response.outcome {
        Outcome::Ok(CommandResult::Render(output)) => match output.format {
            RenderFormat::Json => None,
            RenderFormat::Dot => Some("text/vnd.graphviz; charset=utf-8"),
            RenderFormat::Ascii
            | RenderFormat::AsciiDag
            | RenderFormat::Mermaid => Some("text/plain; charset=utf-8"),
        },
        _ => None,
    };
    match (content_type, response.outcome) {
        (Some(content_type), Outcome::Ok(CommandResult::Render(output))) => (
            [(header::CONTENT_TYPE, content_type)],
            Body::from(output.text.unwrap_or_default()),
        )
            .into_response(),
        (_, outcome) => reply(Response {
            id: response.id,
            outcome,
        }),
    }
}

async fn read(
    State(state): State<AppState>,
    Path(graph): Path<String>,
    Query(params): Params,
    raw: Result<Bytes, BytesRejection>,
) -> HttpResponse {
    let command: Result<Command, Rejected> = (|| {
        let raw = body(raw)?;
        let text = String::from_utf8(Vec::from(raw)).map_err(|err| {
            bad_request(format!("request body is not valid UTF-8: {err}"))
        })?;
        let split: Option<Split> = enum_param(&params, "split")?;
        Ok(Command::Read {
            graph,
            inputs: vec![Input::Text {
                text,
                label: params.get("label").cloned(),
            }],
            split: split.unwrap_or_default(),
            create: bool_param(&params, "create")?,
            save: SavePolicy::Auto,
            steps: false,
            dry_run: false,
        })
    })();
    match command {
        Ok(command) => run(&state, command).await,
        Err(rejected) => rejected.into_response(),
    }
}
