"""Superseded prototype (plan revision 1 → 2): recursive insertion with overlap
closure, starting from an arbitrary host and descending to the minimal one.

`insert_at(R, o, k)` makes `R[o:o+k]` a token: it descends into a child that
contains the span, closes every partially overlapping child by recursively
inserting the intersection into it, composes `N` by recursive cover
projection and adds it to `R`. It reproduces the oracle, but it does not use
the response invariants (the response root is already the narrowest host, so
no descent is needed; decision D1) and closes children one by one instead of
joining consolidated intervals bottom-up (D4); split_join.py replaces it.

Modes:
    python3 descend_closure.py              # insert cases: insert_at(root, span) vs oracle
    python3 descend_closure.py --streaming  # fixture prefixes: read the next input with
                                            # fixed retention (strings), compare with the oracle
"""
import sys
from collections import Counter

from common import (end_path, fmt, graph, insert_cases, maximal,
                    minimal_hosts, scenarios, start_path)


class Graph:
    def __init__(self, V):
        self.V = {k: list(v) for k, v in V.items()}
        self.log = []

    def cover(self, R, a, b):
        """Recursive cover projection of `[a, b)` in `R`."""
        if (a, b) == (0, len(R)):
            return [(0, R)]
        r = []
        for o, c in self.V[R]:
            lo, hi = max(a, o), min(b, o + len(c))
            if lo < hi:
                r += [(lo + x - a, t) for x, t in self.cover(c, lo - o, hi - o)]
        return maximal(r)

    def insert_at(self, R, o, k, depth=0):
        N, ind = R[o:o + k], '  ' * depth
        if k == len(R):
            return
        inner = [(a, c) for a, c in self.V[R] if a <= o and o + k <= a + len(c)]
        if inner:
            a, c = inner[0]
            if (a, c) != (o, N):
                self.insert_at(c, o - a, k, depth)
            return
        for a, c in list(self.V[R]):
            lo, hi = max(o, a), min(o + k, a + len(c))
            if lo < hi and not (o <= a and a + len(c) <= o + k) and hi - lo > 1:
                if N not in self.V:
                    self.log.append(f'{ind}  {c}@{a} crosses {N}@{o} in {R}: '
                                    f'intersection {R[lo:hi]} -> into {c} at {lo - a}')
                self.insert_at(c, lo - a, hi - lo, depth + 1)
        if N not in self.V:
            self.V[N] = self.cover(R, o, o + k)
            self.log.append(f'{ind}new {N} = {fmt(self.V[N])}')
        self.V[R] = maximal([(a, c) for a, c in self.V[R] if not (o <= a and a + len(c) <= o + k)] + [(o, N)])
        self.log.append(f'{ind}host {R} -> {fmt(self.V[R])}')

    def insert(self, T):
        hosts = minimal_hosts(self.V, T)
        if len(hosts) != 1:
            self.log.append(f'!! {T}: {len(hosts)} minimal hosts {hosts}')
            return False
        self.insert_at(hosts[0][0], hosts[0][1], len(T))
        return True

    def read(self, w, retained):
        """Read input `w` with fixed retention: insert the largest retained
        occurring prefix at every position, then publish `w`."""
        if w in self.V:
            return True
        for ch in w:
            self.V.setdefault(ch, [])
        if any(w in K for K in self.V):
            return self.insert(w)
        ok = True
        for x in range(len(w)):
            m = max((j for j in range(x + 1, len(w) + 1) if any(w[x:j] in K for K in self.V)), default=x)
            T = max((w[x:j] for j in range(x + 2, m + 1) if w[x:j] in retained and w[x:j] != w),
                    key=len, default=None)
            if T and T not in self.V:
                ok &= self.insert(T)
        self.V[w] = maximal([(x, t) for x in range(len(w)) for t in self.V if t != w and w.startswith(t, x)])
        self.log.append(f'publish {w} = {fmt(self.V[w])}')
        return ok


def insert_gate():
    c = Counter()
    for name, corpus, V, N, R, o in insert_cases():
        e = o + len(N)
        want = graph(corpus + [N])
        g = Graph(V)
        g.insert_at(R, o, len(N))
        c['ok' if g.V == want else 'mismatch'] += 1
        path_nodes = {v for v, _, _ in start_path(V, R, o, e)[1:] + end_path(V, R, o, e)[1:]}
        c['changed vertices are the root or on the response paths'] += \
            {v for v in V if V[v] != want[v]} - {R} <= path_nodes
        c['cases'] += 1
    for k, v in sorted(c.items()):
        print(f'{k}: {v}')


def streaming_gate():
    c = Counter()
    for name, inputs in scenarios():
        for i in range(1, len(inputs)):
            before, w = inputs[:i], inputs[i]
            want = graph(before + [w])
            g = Graph(graph(before))
            ok = g.read(w, set(want)) and g.V == want
            c['ok' if ok else 'mismatch'] += 1
            if not ok:
                print(f'{before} + {w!r}: mismatch')
                for line in g.log:
                    print('    ' + line)
    for k, v in sorted(c.items()):
        print(f'{k}: {v}')


if __name__ == '__main__':
    if sys.argv[1:] == ['--streaming']:
        streaming_gate()
    elif not sys.argv[1:]:
        insert_gate()
    else:
        sys.exit(__doc__)
