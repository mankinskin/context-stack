"""Shared helpers for the positioned-insert prototypes (see README.md).

A graph is a dict `vertex string -> [(offset, child string)]`, the v3
fixture form: every vertex lists its maximal retained proper occurrences
sorted by offset; atoms have empty lists. Vertex strings name vertices and
let the prototypes compare with the oracle; the algorithms under test only
navigate occurrences (offsets and widths) and use strings for naming new
vertices (`label`). Searches are replaced by brute-force stand-ins
(`minimal_hosts`, `start_path`, `end_path`) that return what positioned
search returns (validated in context-search plan 2.6).
"""
import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
FIXTURES = REPO / 'context-read' / 'tests' / 'fixtures' / 'v3'
NGRAMS = Path(os.environ.get('NGRAMS_BIN', REPO / 'target' / 'release' / 'ngrams'))

_cache = {}


def graph(corpus):
    """Oracle graph of `corpus` (runs the ngrams CLI, cached per corpus)."""
    key = tuple(corpus)
    if key not in _cache:
        if not NGRAMS.exists():
            sys.exit(f'{NGRAMS} not found; build it with: '
                     'cargo build --release -p ngrams --bin ngrams')
        out = subprocess.run([str(NGRAMS), '--', *corpus],
                             capture_output=True, text=True, check=True).stdout
        _cache[key] = {v: [(o, c) for o, c in cs]
                       for v, cs in json.loads(out)['vertices'].items()}
    return _cache[key]


def scenarios():
    """`(name, inputs)` of every v3 fixture scenario."""
    return [(s['name'], s['inputs'])
            for s in json.loads((FIXTURES / 'scenarios.json').read_text())]


def fixture_graph(name):
    """The checked-in v3 fixture graph `name` (equal to `graph(inputs)`)."""
    data = json.loads((FIXTURES / f'{name}.json').read_text())
    return {v: [(o, c) for o, c in cs] for v, cs in data['vertices'].items()}


def fmt(comp):
    return ', '.join(f'{t}@{o}' for o, t in comp)


def maximal(occ):
    """Sort by start (wider first) and keep an occurrence only if it ends after
    every earlier one: drops duplicates and contained occurrences, keeps
    partial overlaps (the oracle's normalization)."""
    occ = sorted(set(occ), key=lambda o: (o[0], -len(o[1])))
    out, end = [], -1
    for x, t in occ:
        if x + len(t) > end:
            out.append((x, t))
            end = x + len(t)
    return out


def brute_cover(s, keys):
    """Maximal occurrences of `keys` (narrower than `s`) in the string `s`."""
    return maximal([(x, k) for k in keys if k != s and len(k) < len(s)
                    for x in range(len(s) - len(k) + 1) if s[x:x + len(k)] == k])


def minimal_hosts(V, T):
    """Occurrences `(vertex, offset)` of the string `T` that lie in no child of
    that vertex. Stand-in for the root and span of a positioned search."""
    return [(K, q) for K in V if len(K) > len(T)
            for q in range(len(K) - len(T) + 1)
            if K[q:q + len(T)] == T
            and not any(o <= q and q + len(T) <= o + len(c) for o, c in V[K])]


def start_path(V, R, p, e):
    """Response start path for span `[p, e)` of `R`: `[(parent, offset, child)]`,
    root first, descending through the covering child to the largest token
    at `p` that ends at or before `e`."""
    path, v = [], R
    while True:
        at = [(o, c) for o, c in V[v] if o == p and o + len(c) <= e]
        if at:
            o, c = max(at, key=lambda x: len(x[1]))
            path.append((v, o, c))
            return path
        o, c = [x for x in V[v] if x[0] <= p][-1]
        path.append((v, o, c))
        v, p, e = c, p - o, e - o


def end_path(V, R, s, e):
    """Response end path for span `[s, e)` of `R`: descends to the largest token
    ending at `e` that starts at or after `s`."""
    path, v = [], R
    while True:
        at = [(o, c) for o, c in V[v] if o + len(c) == e and o >= s]
        if at:
            o, c = max(at, key=lambda x: len(x[1]))
            path.append((v, o, c))
            return path
        o, c = [x for x in V[v] if x[0] + len(x[1]) >= e][0]
        path.append((v, o, c))
        v, s, e = c, s - o, e - o


def covering(V, v, p):
    """Last occurrence of `v` with offset <= p (`Composition::covering`)."""
    return [x for x in V[v] if x[0] <= p][-1]


def ending_at(V, v, q):
    """First occurrence of `v` with end >= q (`Composition::ending_at`)."""
    return [x for x in V[v] if x[0] + len(x[1]) >= q][0]


def label(V, comp):
    """Name of a new vertex with composition `comp` (naming only)."""
    s, e = '', 0
    for o, c in comp:
        s += c[max(0, e - o):]
        e = max(e, o + len(c))
    return s


def non_token_substrings(V, min_width=2):
    return sorted({K[a:b] for K in V for a in range(len(K))
                   for b in range(a + min_width, len(K) + 1)} - set(V))


def insert_cases():
    """Every insert case of the plan's gate: for every fixture and every
    substring `N` of a vertex string that is not a vertex, yield
    `(fixture, corpus, graph, N, root, offset)`. `(root, offset)` is the unique
    minimal host occurrence (F3), i.e. the root and span of the response."""
    for name, corpus in scenarios():
        V = graph(corpus)
        for N in non_token_substrings(V):
            hosts = minimal_hosts(V, N)
            assert len(hosts) == 1, ('F3 violated', name, N, hosts)
            R, o = hosts[0]
            yield name, corpus, V, N, R, o


def graph_diff(got, want, limit=4):
    """`[(vertex, got, want)]` for vertices whose composition differs."""
    return [(v, got.get(v), want.get(v)) for v in sorted(set(want) | set(got), key=len)
            if got.get(v) != want.get(v)][:limit]


# Named examples of the plan (Part B, 3.1): minimal corpora of larger strings;
# the oracle derives the repeated children.
EXAMPLES = {
    'insert_shared_vertex': (['acabcabcab', 'bcabcb'], 'abcabca'),
    'insert_periodic': (['ababababa'], 'bababab'),
    'insert_deep_borders': (['baaabababba', 'abaaabbaabba', 'aabbaabbaabbaa'], 'bbaabbaabb'),
    'insert_small_periodic': (['ababab'], 'baba'),
}


def example_args(argv):
    """Parse `--example NAME` or `--query Q CORPUS...`; `None` for the gate."""
    if not argv:
        return None
    if argv[0] == '--example' and len(argv) == 2 and argv[1] in EXAMPLES:
        return EXAMPLES[argv[1]]
    if argv[0] == '--query' and len(argv) >= 3:
        return argv[2:], argv[1]
    sys.exit('usage: [--example ' + '|'.join(EXAMPLES) + ' | --query QUERY CORPUS...]')


def print_case(corpus, N):
    """Print the oracle graph of `corpus` and the response for `N`; return
    `(V, R, o, e)`."""
    V = graph(corpus)
    hosts = minimal_hosts(V, N)
    if N in V or len(hosts) != 1:
        sys.exit(f'{N!r} is a vertex or has no unique minimal host: {hosts}')
    R, o = hosts[0]
    e = o + len(N)
    print(f'corpus {corpus}, query {N!r}')
    for v in sorted(V, key=lambda v: (len(v), v)):
        if V[v]:
            print(f'    {v} = {fmt(V[v])}')
    print(f'response: root {R}, span [{o}, {e})')
    print(f'  start path {start_path(V, R, o, e)}')
    print(f'  end path   {end_path(V, R, o, e)}')
    return V, R, o, e


def print_oracle_check(got, corpus, N):
    want = graph(corpus + [N])
    print('equals oracle(corpus + [query]):', got == want)
    for d in graph_diff(got, want):
        print('    diff', d)
