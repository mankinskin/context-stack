"""Accepted algorithm (plan Part B, decision D4): insertion from a search
response through a split graph and a bottom-up join.

Split (read-only): the root gets the span [o, e); intervals are pushed top-down
in decreasing vertex width (a vertex is processed once all wider vertices are
done, so its interval set is final). Inside a vertex, every two partially
overlapping intervals add their intersection (overlap closure I2). An
interval inside a child is pushed into that child; otherwise its start is
pushed as a postfix into `covering(x)` and its end as a prefix into
`ending_at(y)`. For the root span these pushes are the steps of the response's
start and end paths (checked by the gate).

Join: vertices in increasing width, inside a vertex intervals in increasing
width. An interval inside an original child resolves to that child or to the
child's interval token; otherwise a token is composed from `maximal` of the
pieces: children inside the interval, the interval tokens of crossing
children and the vertex's own smaller interval tokens. A composition already
created in this insertion is reused. Each vertex finally becomes
`maximal(original ∪ own interval tokens)`.

Usage:
    python3 split_join.py                         # gate over all insert cases
    python3 split_join.py --example insert_shared_vertex
    python3 split_join.py --query abcabca acabcabcab bcabcb
"""
import sys
from collections import Counter

from common import (covering, end_path, ending_at, example_args, fmt, graph,
                    graph_diff, insert_cases, label, maximal, print_case,
                    print_oracle_check, start_path)


def close(intervals):
    """Add the intersection of every two partially overlapping intervals."""
    iv, grow = set(intervals), True
    while grow:
        grow = False
        for (a, b) in list(iv):
            for (c, d) in list(iv):
                lo, hi = max(a, c), min(b, d)
                if lo < hi and (lo, hi) not in iv:
                    iv.add((lo, hi))
                    grow = True
    return iv


def split(V, R, o, e):
    """Split graph: vertex -> set of required intervals (relative ranges)."""
    req, done = {R: {(o, e)}}, set()
    while True:
        todo = [v for v in req if v not in done]
        if not todo:
            return req
        v = max(todo, key=len)
        done.add(v)
        req[v] = close(req[v])
        for x, y in req[v]:
            inside = [(a, c) for a, c in V[v] if a <= x and y <= a + len(c)]
            push = []
            if inside:
                a, c = inside[0]
                push.append((c, x - a, y - a))
            else:
                a, c = covering(V, v, x)
                if a < x:
                    push.append((c, x - a, len(c)))
                b, d = ending_at(V, v, y)
                if b + len(d) > y:
                    push.append((d, 0, y - b))
            for c, lo, hi in push:
                if (lo, hi) != (0, len(c)):
                    req.setdefault(c, set()).add((lo, hi))


def join(V, req, log=None, stats=None):
    """Resolve the split graph bottom-up; returns the new graph."""
    orig = V
    V = {v: list(c) for v, c in V.items()}
    st = stats if stats is not None else Counter()
    tok, made = {}, {}
    for v in sorted(req, key=len):
        own = []
        for x, y in sorted(req[v], key=lambda i: (i[1] - i[0], i)):
            inside = [(a, c) for a, c in orig[v] if a <= x and y <= a + len(c)]
            if inside:
                a, c = inside[0]
                t = c if (x - a, y - a) == (0, len(c)) else tok[(c, x - a, y - a)]
            else:
                pieces, skipped = [], []
                for a, c in orig[v]:
                    lo, hi = max(x, a), min(y, a + len(c))
                    if lo >= hi:
                        continue
                    if (lo, hi) == (a, a + len(c)):
                        pieces.append((a - x, c))
                    elif (c, lo - a, hi - a) in tok:
                        pieces.append((lo - x, tok[(c, lo - a, hi - a)]))
                    else:
                        st['skipped crossing child'] += 1
                        skipped.append((lo - x, hi - x))
                pieces += [(a - x, t) for (a, t) in own if x <= a and a + len(t) <= y]
                comp = maximal(pieces)
                for lo, hi in skipped:
                    assert any(a <= lo and hi <= a + len(t) for a, t in comp), \
                        ('skipped crossing child not inside a piece', v, x, y)
                key = tuple(comp)
                if key in made:
                    t = made[key]
                    st['dedup'] += 1
                else:
                    t = label(V, comp)
                    assert t not in V, ('duplicate content', t)
                    V[t] = comp
                    made[key] = t
                    st['new'] += 1
                if log is not None:
                    log.append(f'{v}[{x}:{y}] = {t} = {fmt(comp)}')
            tok[(v, x, y)] = t
            own.append((x, t))
        new = maximal(orig[v] + own)
        if new != orig[v]:
            V[v] = new
            if log is not None:
                log.append(f'    {v} -> {fmt(new)}')
    return V


def insert_match(V, R, o, e, log=None, stats=None):
    return join(V, split(V, R, o, e), log, stats)


def gate():
    c, bad = Counter(), []
    for name, corpus, V, N, R, o in insert_cases():
        e = o + len(N)
        sp, ep = start_path(V, R, o, e), end_path(V, R, o, e)
        req = split(V, R, o, e)
        # the root span's pushes are the response path steps
        primary, ab = True, 0
        for _, off, ch in sp[:-1]:
            ab += off
            primary &= (o - ab, len(ch)) in req.get(ch, ())
        ab = 0
        for _, off, ch in ep[:-1]:
            ab += off
            primary &= (0, e - ab) in req.get(ch, ())
        c['response paths = primary split chains'] += primary
        on_paths = {ch for _, _, ch in sp + ep} | {R}
        c['cases with split nodes off the paths'] += bool(set(req) - on_paths)
        c['split nodes'] += len(req)
        c['max intervals per node'] = max(c['max intervals per node'],
                                          max(len(i) for i in req.values()))
        try:
            got = join(V, req, stats=c)
        except (AssertionError, KeyError) as x:
            c['error'] += 1
            bad.append((name, N, repr(x)))
            continue
        want = graph(corpus + [N])
        c['ok' if got == want else 'mismatch'] += 1
        if got != want:
            bad.append((name, N, graph_diff(got, want)))
        c['cases'] += 1
    for k, v in sorted(c.items()):
        print(f'{k}: {v}')
    for b in bad[:8]:
        print(b)


def trace(corpus, N):
    V, R, o, e = print_case(corpus, N)
    print('split graph:')
    req = split(V, R, o, e)
    for v in sorted(req, key=lambda v: (-len(v), v)):
        print(f'    {v}: {sorted(req[v])}')
    print('join:')
    log = []
    got = join(V, req, log)
    for line in log:
        print('    ' + line)
    print_oracle_check(got, corpus, N)


if __name__ == '__main__':
    args = example_args(sys.argv[1:])
    gate() if args is None else trace(*args)
