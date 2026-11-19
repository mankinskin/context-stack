use context_search::Response;
use context_trace::legacy::*;

/// Outcome of `insert_next_match` — always resolves to a single token + path.
///
/// Each variant carries the matched/created [`IndexWithPath`] and the search
/// [`Response`] for caching, debugging, and downstream visibility.
///
/// # Variants
///
/// - **`Created`** — A new token was inserted via the split+join pipeline.
///   The query extended beyond what was known in the graph.
/// - **`Complete`** — The query was fully consumed by an existing token.
///   No insertion was needed (idempotent match).
/// - **`NoExpansion`** — The search found an existing token at the start of
///   the query, but the query extends beyond it. No new token was created.
///   The caller should advance by the returned token's width and retry.
#[derive(Debug, Clone)]
pub enum InsertOutcome {
    /// Newly created via split+join pipeline.
    /// The query extended beyond what was known; a new token was inserted.
    Created {
        result: IndexWithPath,
        response: Response,
    },

    /// Full match already existed — the query was fully consumed by an
    /// existing token. No insertion was needed.
    Complete {
        result: IndexWithPath,
        response: Response,
    },

    /// No expansion: the search found an existing token at the start of
    /// the query, but the query extends beyond it. No new token was created.
    /// The caller should advance by the returned token's width and try again.
    NoExpansion {
        result: IndexWithPath,
        response: Response,
    },
}

impl InsertOutcome {
    /// The matched/created token + cursor path (all variants carry this).
    pub fn result(&self) -> &IndexWithPath {
        match self {
            InsertOutcome::Created { result, .. } => result,
            InsertOutcome::Complete { result, .. } => result,
            InsertOutcome::NoExpansion { result, .. } => result,
        }
    }

    /// Consume into the [`IndexWithPath`].
    pub fn into_result(self) -> IndexWithPath {
        match self {
            InsertOutcome::Created { result, .. } => result,
            InsertOutcome::Complete { result, .. } => result,
            InsertOutcome::NoExpansion { result, .. } => result,
        }
    }

    /// The token (shorthand for `result().index`).
    pub fn token(&self) -> Token {
        self.result().index
    }

    /// The search response (for caching, debugging, trace inspection).
    pub fn response(&self) -> &Response {
        match self {
            InsertOutcome::Created { response, .. } => response,
            InsertOutcome::Complete { response, .. } => response,
            InsertOutcome::NoExpansion { response, .. } => response,
        }
    }

    /// Whether a new token was created (split+join pipeline ran).
    pub fn is_expanded(&self) -> bool {
        matches!(self, InsertOutcome::Created { .. })
    }

    /// Whether the query was fully consumed by an existing token.
    pub fn is_complete(&self) -> bool {
        matches!(self, InsertOutcome::Complete { .. })
    }

    /// Whether no expansion occurred (starting token is the best match,
    /// but query extends beyond it).
    pub fn is_no_expansion(&self) -> bool {
        matches!(self, InsertOutcome::NoExpansion { .. })
    }
}

/// An [`InsertOutcome`] together with the edits its publication applied.
///
/// Use [`Publication::translate`](crate::insert::publication::Publication::translate)
/// to carry graph locations known before materialization across it.
#[derive(Debug, Clone)]
pub struct Materialization {
    pub outcome: InsertOutcome,
    pub publication: crate::insert::publication::Publication,
}

impl Materialization {
    /// The first token of the materialized query.
    pub fn query_start(&self) -> Token {
        self.outcome.response().query_cursor().path().pattern_root_pattern()
            [0]
    }

    /// Frames connecting the materialized token to [`Self::query_start`]
    /// through first children, projected from the accepted search.
    ///
    /// - A reused whole-root result starts with the occurrence recorded by
    ///   the search ([`Response::entire_root_start_path`]). The search
    ///   simplified away the trailing frames, which all lie at offset 0, so
    ///   they are completed by [`prefix_frames`](crate::overlap::prefix_frames).
    /// - A created result is entered through first children of vertices
    ///   created by its publication until an existing vertex is reached; the
    ///   first-child edges the search recorded below it are translated
    ///   through the publication, and the unrecorded border frames below
    ///   those (all at offset 0) are completed as above.
    ///
    /// Returns `None` if the evidence does not connect the result to the
    /// query start.
    pub fn prefix_path(
        &self,
        graph: &Hypergraph,
    ) -> Option<Vec<ChildLocation>> {
        let target = self.query_start();
        let token = self.outcome.token();
        if token == target {
            // Also covers the search's fallback for an unextended start
            // token, whose root location is a placeholder.
            return Some(Vec::new());
        }
        let response = self.outcome.response();
        if let Some(mut path) = response.entire_root_start_path() {
            assert_eq!(
                response.root_token(),
                token,
                "reused result must be the matched root"
            );
            let leaf = boundary_target(graph, token, &path, true);
            path.extend(crate::overlap::prefix_frames(graph, leaf, target)?);
            return Some(path);
        }
        self.enter_prefix(graph, &response.cache, token, target)
    }

    fn enter_prefix(
        &self,
        graph: &Hypergraph,
        cache: &TraceCache,
        token: Token,
        target: Token,
    ) -> Option<Vec<ChildLocation>> {
        if token == target {
            return Some(Vec::new());
        }
        if !self.publication.created(token) {
            let (traced, leaf) = traced_prefix(cache, token, target);
            let mut path = self.publication.translate(graph, &traced, leaf, true);
            path.extend(crate::overlap::prefix_frames(graph, leaf, target)?);
            return Some(path);
        }
        graph
            .expect_child_patterns(token)
            .iter()
            .filter(|(_, pattern)| pattern[0].width() >= target.width())
            .find_map(|(&id, pattern)| {
                let mut rest =
                    self.enter_prefix(graph, cache, pattern[0], target)?;
                rest.insert(0, ChildLocation::new(token, id, 0));
                Some(rest)
            })
    }
}

/// Follow first-child edges recorded by a search from `root` towards
/// `target`, returning the recorded frames and the vertex they reach.
///
/// The search records only edges it needs for splitting; children matched
/// entirely at the start border are not recorded, so the walk stops at the
/// deepest recorded vertex. Prefers the widest traced child at each level;
/// every first-child path identifies the same occurrence at offset 0.
fn traced_prefix(
    cache: &TraceCache,
    root: Token,
    target: Token,
) -> (Vec<ChildLocation>, Token) {
    let mut token = root;
    let mut path = Vec::new();
    while token != target {
        let Some(entry) = cache.entries.get(&token.vertex_index()) else {
            break;
        };
        let Some((location, child)) = entry
            .bottom_up
            .values()
            .chain(entry.top_down.values())
            .flat_map(|position| position.bottom())
            .filter(|(key, sub)| {
                sub.sub_index() == 0 && key.index.width() >= target.width()
            })
            .map(|(key, sub)| {
                (
                    ChildLocation::new(token, sub.pattern_id(), 0),
                    key.index,
                )
            })
            .max_by_key(|(_, child)| child.width())
        else {
            break;
        };
        path.push(location);
        token = child;
    }
    (path, token)
}
