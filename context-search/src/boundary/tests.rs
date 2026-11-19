//! Boundary search on every v3 fixture against brute-force string matching
//! over the retained set (the fixture's vertex keys).

use context_trace::{
    BaseGraphKind,
    Token,
    positioned::{
        PositionedGraph,
        fixtures::v3_fixtures,
    },
};
use pretty_assertions::assert_eq;

use super::{
    Boundary,
    BoundaryOccurrence,
    boundary_occurrences,
    postfixes,
    prefixes,
};

type Graph = PositionedGraph<BaseGraphKind>;

/// `(graph, [(content, token)])` per fixture.
fn retained_sets() -> Vec<(String, Graph, Vec<(Vec<char>, Token)>)> {
    v3_fixtures()
        .into_iter()
        .map(|(name, fixture)| {
            let graph = Graph::from_interchange(&fixture.graph).unwrap();
            let retained = fixture
                .graph
                .vertices
                .keys()
                .map(|content| {
                    let atoms: Vec<char> = content.chars().collect();
                    let token = graph.find(&atoms).unwrap();
                    (atoms, token)
                })
                .collect();
            (name, graph, retained)
        })
        .collect()
}

/// Tokens other than `owner` whose content satisfies `pred`, longest first.
fn expected(
    retained: &[(Vec<char>, Token)],
    owner: Token,
    pred: impl Fn(&[char]) -> bool,
) -> Vec<Token> {
    let mut tokens: Vec<Token> = retained
        .iter()
        .filter(|(atoms, token)| *token != owner && pred(atoms))
        .map(|(_, token)| *token)
        .collect();
    tokens.sort_by(|a, b| b.width.cmp(&a.width).then(a.index.cmp(&b.index)));
    tokens
}

/// Each result's path is stored in the graph, descends from `owner` to the
/// token and places it at `offset`.
fn check_evidence(
    context: &str,
    graph: &Graph,
    owner: Token,
    found: &[BoundaryOccurrence],
    offset: impl Fn(Token) -> usize,
) -> Vec<Token> {
    for occurrence in found {
        let path = &occurrence.path;
        assert!(graph.contains_path(path), "{context}: {path:?}");
        assert_eq!(path.root(), owner.index, "{context}");
        assert_eq!(path.target(), occurrence.token.index, "{context}");
        assert_eq!(
            usize::from(occurrence.offset()),
            offset(occurrence.token),
            "{context}: {path:?}"
        );
    }
    found.iter().map(|o| o.token).collect()
}

#[test]
fn prefixes_and_postfixes_equal_retained_prefixes_and_postfixes() {
    let mut owners = 0;
    for (name, graph, retained) in retained_sets() {
        for (owner_atoms, owner) in &retained {
            let context = format!("{name}: {:?}", owner_atoms.iter().collect::<String>());
            let width = owner_atoms.len();
            let found = prefixes(&graph, owner.index).unwrap();
            assert_eq!(
                check_evidence(&context, &graph, *owner, &found, |_| 0),
                expected(&retained, *owner, |atoms| owner_atoms.starts_with(atoms)),
                "{context}: prefixes"
            );
            let found = postfixes(&graph, owner.index).unwrap();
            assert_eq!(
                check_evidence(&context, &graph, *owner, &found, |t| width - t.width.0),
                expected(&retained, *owner, |atoms| owner_atoms.ends_with(atoms)),
                "{context}: postfixes"
            );
            owners += 1;
        }
    }
    assert!(owners > 100, "{owners}");
}

#[test]
fn boundary_occurrences_at_every_position_equal_brute_force() {
    for (name, graph, retained) in retained_sets() {
        for (owner_atoms, owner) in &retained {
            let width = owner_atoms.len();
            for position in 0..=width {
                let context = format!(
                    "{name}: {:?} at {position}",
                    owner_atoms.iter().collect::<String>()
                );
                let found = boundary_occurrences(
                    &graph,
                    owner.index,
                    position.into(),
                    Boundary::Start,
                )
                .unwrap();
                assert_eq!(
                    check_evidence(&context, &graph, *owner, &found, |_| position),
                    expected(&retained, *owner, |atoms| {
                        owner_atoms[position..].starts_with(atoms)
                    }),
                    "{context}: start"
                );
                let found =
                    boundary_occurrences(&graph, owner.index, position.into(), Boundary::End)
                        .unwrap();
                assert_eq!(
                    check_evidence(&context, &graph, *owner, &found, |t| position
                        - t.width.0),
                    expected(&retained, *owner, |atoms| {
                        owner_atoms[..position].ends_with(atoms)
                    }),
                    "{context}: end"
                );
            }
            assert_eq!(
                boundary_occurrences(&graph, owner.index, (width + 1).into(), Boundary::Start),
                None
            );
        }
    }
}

/// `abcde = [abc@0, bcd@1, cde@2]`: the postfix `e` is only reachable
/// through `cde`, `bc@1` only below `abc` or `bcd`.
#[test]
fn boundary_paths_descend_through_children() {
    let (_, graph, retained) = retained_sets()
        .into_iter()
        .find(|(name, ..)| name == "seam_bcd")
        .unwrap();
    let token = |content: &str| {
        retained
            .iter()
            .find(|(atoms, _)| atoms.iter().collect::<String>() == content)
            .unwrap()
            .1
    };
    let abcde = token("abcde");
    let names = |found: Vec<BoundaryOccurrence>| -> Vec<String> {
        found
            .into_iter()
            .map(|o| graph.content(o.token.index).unwrap().into_iter().collect())
            .collect()
    };
    assert_eq!(names(prefixes(&graph, abcde.index).unwrap()), ["abc", "a"]);
    assert_eq!(names(postfixes(&graph, abcde.index).unwrap()), ["cde", "e"]);
    let at_1 =
        boundary_occurrences(&graph, abcde.index, 1.into(), Boundary::Start).unwrap();
    assert_eq!(names(at_1.clone()), ["bcd", "bc", "b"]);
    assert_eq!(at_1[1].path.steps().len(), 2, "bc is found below a child");
}
