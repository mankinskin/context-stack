//! JSON shapes of the wire protocol (plan D6, 1.4).

use context_api::{
    types::*,
    ApiError,
    Command,
    CommandResult,
    Outcome,
    Request,
    Response,
};
use ::serde_json::{
    json,
    Value,
};

use crate::common::*;

fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap_or_else(|| panic!("not an object: {value}"))
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    keys
}

#[test]
fn request_with_id_round_trips() {
    let request = Request::with_id(
        7,
        Command::Read {
            graph: "g".into(),
            inputs: vec![
                Input::Text {
                    text: "ab".into(),
                    label: Some("t".into()),
                },
                Input::File { path: "x.txt".into() },
            ],
            split: Split::Lines,
            create: true,
            save: SavePolicy::Defer,
            steps: true,
            dry_run: false,
        },
    );
    let text = ::serde_json::to_string(&request).unwrap();
    let value: Value = ::serde_json::from_str(&text).unwrap();
    assert_eq!(value["id"], 7);
    assert_eq!(value["command"], "read");
    assert_eq!(value["inputs"][0], json!({"kind": "text", "text": "ab", "label": "t"}));
    assert_eq!(value["split"], "lines");
    assert_eq!(::serde_json::from_str::<Request>(&text).unwrap(), request);

    let without_id = ::serde_json::to_value(Request::new(Command::List)).unwrap();
    assert_eq!(without_id, json!({"command": "list"}));
}

#[test]
fn minimal_search_request_takes_defaults() {
    let request: Request =
        ::serde_json::from_str(r#"{"id":1,"command":"search","graph":"g","query":"ab"}"#).unwrap();
    assert_eq!(
        request,
        Request::with_id(
            1,
            Command::Search {
                graph: "g".into(),
                query: "ab".into(),
                view: SearchView::Summary,
                limits: Limits::default(),
            }
        )
    );
    let request: Request = ::serde_json::from_str(
        r#"{"command":"search","graph":"g","query":"ab","view":"occurrences","limits":{"limit":3}}"#,
    )
    .unwrap();
    let Command::Search { view, limits, .. } = request.command else {
        panic!("{request:?}");
    };
    assert_eq!(view, SearchView::Occurrences);
    assert_eq!(limits.limit, Some(3));
    assert_eq!(limits.max_visited, None);
}

#[test]
fn ok_response_shape() {
    let token = TokenInfo {
        index: 3,
        width: 2,
        label: "ab".into(),
        label_truncated: false,
    };
    let response = Response {
        id: Some(1),
        outcome: Outcome::Ok(CommandResult::Search(SearchResult {
            graph: "g".into(),
            query: "ab".into(),
            outcome: SearchOutcome::Vertex { token },
            path: None,
            contexts: None,
            occurrences: None,
            parents: None,
        })),
    };
    let value = ::serde_json::to_value(&response).unwrap();
    assert_eq!(keys(&value), ["id", "ok"]);
    assert_eq!(value["id"], 1);
    assert_eq!(keys(&value["ok"]), ["search"]);
    let search = &value["ok"]["search"];
    assert_eq!(search["query"], "ab");
    assert_eq!(search["outcome"]["kind"], "vertex");
    assert_eq!(search["outcome"]["token"], json!({"index": 3, "width": 2, "label": "ab"}));
    assert_eq!(::serde_json::from_value::<Response>(value).unwrap(), response);
}

#[test]
fn error_response_shape() {
    let response = Response::from_result(
        Some(1),
        Err(ApiError::GraphNotFound { graph: "g".into() }),
    );
    let value = ::serde_json::to_value(&response).unwrap();
    assert_eq!(keys(&value), ["error", "id"]);
    assert_eq!(value["error"]["kind"], "not_found");
    assert!(value["error"]["message"].as_str().unwrap().contains("'g'"));
    assert!(value["error"]["hint"].is_string());
    assert_eq!(::serde_json::from_value::<Response>(value).unwrap(), response);
}

#[test]
fn execute_echoes_the_request_id() {
    let env = Env::new();
    let response = env.session.execute(Request::with_id(42, Command::List));
    assert_eq!(response.id, Some(42));
    let response = env.session.execute(Request::with_id(
        43,
        Command::Info {
            graph: "missing".into(),
        },
    ));
    assert_eq!(response.id, Some(43));
    assert!(!response.is_ok());
}
