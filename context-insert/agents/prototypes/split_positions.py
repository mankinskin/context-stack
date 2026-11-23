"""Accepted algorithm (insert plan revision 3): insertion from a search
response through a split graph of split positions and a bottom-up join.

Split (read-only): the root gets the start border `o` (role S) and the end
border `e` (role E). Vertices are processed in decreasing width (a vertex is
processed once all wider vertices are done, so its positions are final). A
position `p` of role `r` in `v` is pushed as `(p - a, r)` into every child
occurrence `c@a` of `v` that strictly contains it (`a < p < a + |c|`), the
legacy rule "propagate a split into every child containing it". Positions on
the response's start and end paths are read from the path edges; the other
crossing children come from the composition.

Intervals are derived per vertex, never stored: the root needs its target
`[o, e)`; any other vertex needs the postfix `[s, w)` for every S position,
the prefix `[0, t)` for every E position and the overlap `[s, t)` of every
such postfix and prefix with `s < t`.

Join: vertices in increasing width, inside a vertex intervals in increasing
width. An interval inside an original child resolves to that child or to the
child's interval token; otherwise a token is composed from `maximal` of the
pieces: children inside the interval, the interval tokens of crossing
children (they always exist) and the vertex's own smaller interval tokens.
Each changed vertex finally becomes `maximal(original + own new tokens)`.

Usage:
    python3 split_positions.py                          # gate over all insert cases
    python3 split_positions.py --no-overlaps            # gate without overlap intervals (fails)
    python3 split_positions.py --paths-only             # gate pushing only along covering/ending_at (fails)
    python3 split_positions.py --example insert_shared_vertex
    python3 split_positions.py --query abcabca acabcabcab bcabcb
"""
import sys
from collections import Counter

from common import (covering, end_path, ending_at, example_args, fmt, graph,
                    graph_diff, insert_cases, label, maximal, print_case,
                    print_oracle_check, start_path)

START, END = 'S', 'E'


def split(V, R, o, e, paths_only=False):
    """Split graph: vertex -> set of (position, role). `paths_only` (rejected
    variant) pushes S only into `covering(p)` and E only into `ending_at(p)`."""
    pos, done = {R: {(o, START), (e, END)}}, set()
    while True:
        todo = [v for v in pos if v not in done]
        if not todo:
            return pos
        v = max(todo, key=len)
        done.add(v)
        for p, r in pos[v]:
            children = V[v]
            if paths_only:
                children = [covering(V, v, p) if r == START else ending_at(V, v, p)]
            for a, c in children:
                if a < p < a + len(c):
                    pos.setdefault(c, set()).add((p - a, r))


def intervals(pos, v, R, overlaps=True):
    """Required intervals of `v`, derived from its positions."""
    S = sorted(p for p, r in pos[v] if r == START)
    E = sorted(p for p, r in pos[v] if r == END)
    if v == R:
        return [(S[0], E[0])]
    iv = {(s, len(v)) for s in S} | {(0, t) for t in E}
    if overlaps:
        iv |= {(s, t) for s in S for t in E if s < t}
    return sorted(iv, key=lambda i: (i[1] - i[0], i))


def join(V, pos, R, log=None, stats=None, overlaps=True):
    """Resolve the split graph bottom-up; returns the new graph."""
    orig = V
    V = {v: list(c) for v, c in V.items()}
    st = stats if stats is not None else Counter()
    tok, made = {}, {}
    for v in sorted(pos, key=len):
        own = []
        for x, y in intervals(pos, v, R, overlaps):
            inside = [(a, c) for a, c in orig[v] if a <= x and y <= a + len(c)]
            if inside:
                a, c = inside[0]
                t = c if (x - a, y - a) == (0, len(c)) else tok[(c, x - a, y - a)]
                st['resolved inside a child'] += 1
            else:
                pieces = []
                for a, c in orig[v]:
                    lo, hi = max(x, a), min(y, a + len(c))
                    if lo >= hi:
                        continue
                    if (lo, hi) == (a, a + len(c)):
                        pieces.append((a - x, c))
                    else:
                        pieces.append((lo - x, tok[(c, lo - a, hi - a)]))
                pieces += [(a - x, t) for (a, t) in own if x <= a and a + len(t) <= y]
                comp = maximal(pieces)
                assert len(comp) >= 2, ('composition needs two occurrences', v, x, y)
                key = tuple(comp)
                if key in made:
                    st['created twice'] += 1
                    t = made[key]
                else:
                    t = label(V, comp)
                    assert t not in V, ('duplicate content', t)
                    V[t] = comp
                    made[key] = t
                    st['new'] += 1
                own.append((x, t))
                if log is not None:
                    log.append(f'{v}[{x}:{y}] = {t} = {fmt(comp)}')
            tok[(v, x, y)] = t
        new = maximal(orig[v] + own)
        if new != orig[v]:
            V[v] = new
            st['changed vertices'] += 1
            if log is not None:
                log.append(f'    {v} -> {fmt(new)}')
    return V


def path_positions(V, R, o, e):
    """Positions given by the response paths' edges: `(child, position, role)`."""
    out = set()
    for path, border, role in ((start_path(V, R, o, e), o, START),
                               (end_path(V, R, o, e), e, END)):
        ab = 0
        for _, off, ch in path[:-1]:
            ab += off
            out.add((ch, border - ab, role))
    return out


def gate(overlaps=True, paths_only=False):
    c, bad = Counter({'created twice': 0}), []
    for name, corpus, V, N, R, o in insert_cases():
        e = o + len(N)
        pos = split(V, R, o, e, paths_only)
        found = {(v, p, r) for v, ps in pos.items() if v != R for p, r in ps}
        on_paths = path_positions(V, R, o, e)
        assert on_paths <= found, ('path edge without split position', name, N)
        c['split positions'] += len(found)
        c['split positions from response path edges'] += len(on_paths)
        c['cases with positions off the response paths'] += bool(found - on_paths)
        c['split nodes'] += len(pos)
        c['off-path positions bounding a composed interval'] += sum(
            (x in {p for p, r in pos[v] if r == START and (v, p, r) not in on_paths}
             or y in {p for p, r in pos[v] if r == END and (v, p, r) not in on_paths})
            and not any(a <= x and y <= a + len(ch) for a, ch in V[v])
            for v in pos if v != R for x, y in intervals(pos, v, R, overlaps))
        for v, ps in pos.items():
            if v != R:
                roles = Counter(r for _, r in ps)
                c['max S positions per node'] = max(c['max S positions per node'], roles[START])
                c['max E positions per node'] = max(c['max E positions per node'], roles[END])
        try:
            got = join(V, pos, R, stats=c, overlaps=overlaps)
        except (AssertionError, KeyError) as x:
            c['error'] += 1
            bad.append((name, N, repr(x)))
            continue
        want = graph(corpus + [N])
        c['ok' if got == want else 'mismatch'] += 1
        if got != want:
            bad.append((name, N, graph_diff(got, want)))
            continue
        new = set(want) - set(V)
        c['new tokens that are not N, a prefix or a postfix of N'] += sum(
            not (N.startswith(t) or N.endswith(t)) for t in new)
        changed = {v for v in V if V[v] != want[v]}
        c['changed vertices that are not split nodes'] += len(changed - set(pos))
        c['cases'] += 1
    for k, v in sorted(c.items()):
        print(f'{k}: {v}')
    for b in bad[:8]:
        print(b)


def trace(corpus, N):
    V, R, o, e = print_case(corpus, N)
    pos = split(V, R, o, e)
    on_paths = path_positions(V, R, o, e)
    print('split graph (position:role, * = not on a response path):')
    for v in sorted(pos, key=lambda v: (-len(v), v)):
        marks = [f'{p}:{r}' + ('' if v == R or (v, p, r) in on_paths else '*')
                 for p, r in sorted(pos[v])]
        print(f'    {v}: {", ".join(marks)}  intervals {intervals(pos, v, R)}')
    print('join:')
    log = []
    got = join(V, pos, R, log)
    for line in log:
        print('    ' + line)
    print_oracle_check(got, corpus, N)


if __name__ == '__main__':
    argv = sys.argv[1:]
    if argv == ['--no-overlaps']:
        gate(overlaps=False)
    elif argv == ['--paths-only']:
        gate(paths_only=True)
    else:
        args = example_args(argv)
        gate() if args is None else trace(*args)
