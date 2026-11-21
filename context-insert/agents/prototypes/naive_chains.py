"""Rejected first response-only attempt: walk the response's start path and end
path bottom-up as they were before the insertion, without re-navigating.

Each start-path node gains its postfix token, each end-path node its prefix
token, then the root gains `N`. Fails when one vertex lies on both paths
(periodic strings such as `baba` in `ababab`): the second chain does not see
the first chain's edits, so the shared intersection is never created. Kept as
the counterexample that motivated two_chains.py and split_join.py.

Usage:
    python3 naive_chains.py                         # gate (expected to fail)
    python3 naive_chains.py --example insert_small_periodic
"""
import sys
from collections import Counter

from common import (end_path, example_args, fmt, graph, graph_diff,
                    insert_cases, label, print_case, print_oracle_check,
                    start_path)


def insert_match(V, R, o, k, sp, ep, log=None):
    """`sp`/`ep`: response paths `[(parent, offset, child)]`, root first."""
    V = {v: list(c) for v, c in V.items()}
    e, made = o + k, {}

    def new(comp):
        key = tuple(comp)
        if key in made:
            return made[key]
        t = label(V, comp)
        assert t not in V, ('duplicate content', t)
        V[t] = comp
        made[key] = t
        return t

    def chain(path):
        """(vertex, absolute offset) of the path nodes below the root,
        bottom-up, and the path leaf."""
        out, ab = [], 0
        for _, off, ch in path:
            ab += off
            out.append((ch, ab))
        return out[:-1][::-1], path[-1][2]

    nodes, X = chain(sp)
    for v, av in nodes:
        p = o - av
        if av + len(v) - o == len(X):
            continue
        comp = [(0, X)] + [(a - p, c) for a, c in V[v] if a > p]
        X = new(comp)
        V[v] = [(a, c) for a, c in V[v] if a < p] + [(p, X)]
        if log is not None:
            log.append(f'start  {v}: +{X}@{p}  {X} = {fmt(comp)}  -> {v} = {fmt(V[v])}')
    nodes, Y = chain(ep)
    for v, av in nodes:
        q = e - av
        if q == len(Y):
            continue
        comp = [(a, c) for a, c in V[v] if a + len(c) < q] + [(q - len(Y), Y)]
        Y = new(comp)
        V[v] = [(0, Y)] + [(a, c) for a, c in V[v] if a + len(c) > q]
        if log is not None:
            log.append(f'end    {v}: +{Y}@0  {Y} = {fmt(comp)}  -> {v} = {fmt(V[v])}')
    comp = sorted({(0, X)} | {(a - o, c) for a, c in V[R] if a > o and a + len(c) < e} | {(k - len(Y), Y)})
    N = new(comp)
    V[R] = sorted([(a, c) for a, c in V[R] if not (o <= a and a + len(c) <= e)] + [(o, N)])
    if log is not None:
        log.append(f'root   {R}: +{N}@{o}  {N} = {fmt(comp)}  -> {R} = {fmt(V[R])}')
    return V


def gate():
    c, bad = Counter(), []
    for name, corpus, V, N, R, o in insert_cases():
        e = o + len(N)
        try:
            got = insert_match(V, R, o, len(N), start_path(V, R, o, e), end_path(V, R, o, e))
        except AssertionError as x:
            c['error'] += 1
            bad.append((name, N, str(x)))
            continue
        want = graph(corpus + [N])
        c['ok' if got == want else 'mismatch'] += 1
        if got != want:
            bad.append((name, N, graph_diff(got, want, 3)))
    for k, v in sorted(c.items()):
        print(f'{k}: {v}')
    for b in bad[:5]:
        print(b)


def trace(corpus, N):
    V, R, o, e = print_case(corpus, N)
    log = []
    try:
        got = insert_match(V, R, o, len(N), start_path(V, R, o, e), end_path(V, R, o, e), log)
    except AssertionError as x:
        got = None
        log.append(f'error: {x}')
    for line in log:
        print('    ' + line)
    if got is not None:
        print_oracle_check(got, corpus, N)


if __name__ == '__main__':
    args = example_args(sys.argv[1:])
    gate() if args is None else trace(*args)
