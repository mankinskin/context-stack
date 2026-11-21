"""Rejected alternative (plan decision D4): close the start border and then the
end border as two independent chains, applying edits in place.

`postfix(v, p)` makes `v[p:]` a token maximal in `v` (via `covering`),
`prefix(v, q)` makes `v[:q]` one (via `ending_at`); both recurse to the
response path leaves, then `N` is composed from the two chain tops and the
root's children inside the span. Reproduces the oracle, but a vertex on both
paths is composed twice (its larger tokens are built before the smaller
intersection inside them exists), and after the first chain the response's
end path is stale and must be re-navigated or translated. The gate also
reports those cases and checks that the end-first order gives the same graph.

Usage:
    python3 two_chains.py                 # gate + staleness statistics
    python3 two_chains.py --example insert_shared_vertex
    python3 two_chains.py --query abcabca acabcabcab bcabcb
"""
import sys
from collections import Counter

from common import (covering, end_path, ending_at, example_args, fmt, graph,
                    graph_diff, insert_cases, label, print_case,
                    print_oracle_check, start_path)


class Chains:
    def __init__(self, V, log=None):
        self.V = {v: list(c) for v, c in V.items()}
        self.made, self.log, self.stats = {}, log, Counter()

    def new(self, comp):
        key = tuple(comp)
        if key in self.made:
            self.stats['dedup'] += 1
            return self.made[key]
        t = label(self.V, comp)
        assert t not in self.V, ('duplicate content', t)
        self.V[t] = comp
        self.made[key] = t
        return t

    def postfix(self, v, p, d=0):
        a, c = covering(self.V, v, p)
        t = c if a == p else self.postfix(c, p - a, d + 1)
        if a + len(c) == len(v):
            return t
        comp = [(0, t)] + [(x - p, u) for x, u in self.V[v] if x > p]
        X = self.new(comp)
        self.V[v] = [(x, u) for x, u in self.V[v] if x < p] + [(p, X)]
        if self.log is not None:
            self.log.append(f'{"  " * d}postfix {v}[{p}:] = {X} = {fmt(comp)};  {v} -> {fmt(self.V[v])}')
        return X

    def prefix(self, v, q, d=0):
        a, c = ending_at(self.V, v, q)
        t = c if a + len(c) == q else self.prefix(c, q - a, d + 1)
        if a == 0:
            return t
        comp = [(x, u) for x, u in self.V[v] if x + len(u) < q] + [(a, t)]
        Y = self.new(comp)
        self.V[v] = [(0, Y)] + [(x, u) for x, u in self.V[v] if x + len(u) > q]
        if self.log is not None:
            self.log.append(f'{"  " * d}prefix  {v}[:{q}] = {Y} = {fmt(comp)};  {v} -> {fmt(self.V[v])}')
        return Y

    def start_chain(self, R, o):
        a, c = covering(self.V, R, o)
        return c if a == o else self.postfix(c, o - a)

    def end_chain(self, R, e):
        b, d = ending_at(self.V, R, e)
        return d if b + len(d) == e else self.prefix(d, e - b)

    def finish(self, R, o, e, X, Y):
        comp = sorted({(0, X)} | {(x - o, u) for x, u in self.V[R] if x > o and x + len(u) < e}
                      | {(e - o - len(Y), Y)})
        N = self.new(comp)
        self.V[R] = sorted([(x, u) for x, u in self.V[R] if not (o <= x and x + len(u) <= e)] + [(o, N)])
        if self.log is not None:
            self.log.append(f'root    {R}[{o}:{e}] = {N} = {fmt(comp)};  {R} -> {fmt(self.V[R])}')
        return N

    def insert_match(self, R, o, e, end_first=False):
        if end_first:
            Y = self.end_chain(R, e)
            X = self.start_chain(R, o)
            Y = end_path(self.V, R, o, e)[-1][2]
        else:
            X = self.start_chain(R, o)
            Y = self.end_chain(R, e)
        return self.finish(R, o, e, X, Y)


def gate():
    c, bad = Counter(), []
    for name, corpus, V, N, R, o in insert_cases():
        e = o + len(N)
        want = graph(corpus + [N])
        sp, ep = start_path(V, R, o, e), end_path(V, R, o, e)
        c['vertex on both response paths'] += bool({v for v, _, _ in sp[1:]} & {v for v, _, _ in ep[1:]})
        g = Chains(V)
        g.start_chain(R, o)
        c['end path stale after the start chain'] += end_path(g.V, R, o, e) != ep
        for end_first in (False, True):
            g = Chains(V)
            try:
                g.insert_match(R, o, e, end_first)
            except AssertionError as x:
                c['error'] += 1
                bad.append((name, N, end_first, str(x)))
                continue
            key = 'end-first ok' if end_first else 'ok'
            c[key if g.V == want else key + ' mismatch'] += 1
            c['cases with reuse'] += g.stats['dedup'] > 0
            if g.V != want:
                bad.append((name, N, end_first, graph_diff(g.V, want)))
        c['cases'] += 1
    for k, v in sorted(c.items()):
        print(f'{k}: {v}')
    for b in bad[:8]:
        print(b)


def trace(corpus, N):
    V, R, o, e = print_case(corpus, N)
    log = []
    g = Chains(V, log)
    g.insert_match(R, o, e)
    print('chains (start, then end):')
    for line in log:
        print('    ' + line)
    print_oracle_check(g.V, corpus, N)


if __name__ == '__main__':
    args = example_args(sys.argv[1:])
    gate() if args is None else trace(*args)
