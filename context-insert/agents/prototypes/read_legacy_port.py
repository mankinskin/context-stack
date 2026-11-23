"""Reader prototype R0 (reader plan): the legacy reader loop on positioned
compositions.

The loop is `context-read/src/legacy` unchanged; only the root edits differ:

- segments: atoms seen for the first time are created at once; the input is
  split into runs of unknown and known atoms. Unknown atoms are appended.
- known run, one step at a time, each committed before the next:
  1. expansion (`expand_root`, only after something of this run was
     consumed): walk the root's postfix tokens largest first (the chain of
     last children), skipping postfixes wider than the consumed part of the
     run. Search `[postfix, rest of run]`; the first match longer than the
     postfix is accepted, inserted with `insert_match` if it is not a token
     (expansion `E`), and integrated into the root at offset
     `|root| - |postfix|` (closure).
  2. otherwise forward match (`match_forward`): the longest prefix of the
     rest of the run that occurs, inserted if it is not a token; the root
     adopts `[root, t]` if that is a token, else `t` is appended.
- root: the input's own vertex. An owned root (created by this read,
  parentless) grows in place; an adopted token is never edited, a new owned
  root `[root, ...]` is created instead.

Positioned closure: integrating `E@o` is `maximal(comp(root) + [E@o])` for
an owned root and `maximal([root@0, E@o])` for an adopted one; no complement
or intermediate token is built. The prototype checks every root edit
against the brute-force complete composition (G2) and the overlap closure
(G4), and every completed input against the oracle graph.

Searches are brute-force stand-ins over vertex strings (longest occurring
prefix, unique minimal host), as in `common.py`.

Usage:
    python3 read_legacy_port.py                 # gate over all fixture scenarios
    python3 read_legacy_port.py --no-seam-bound # gate, probing postfixes wider than the consumed run
    python3 read_legacy_port.py abc abcabca     # trace reading these inputs in order
"""
import sys
from collections import Counter

from common import brute_cover, fmt, graph, graph_diff, maximal, minimal_hosts, scenarios
from split_positions import join, split

SEAM_BOUND = True


def occurs(V, s):
    return any(s in K for K in V)


def longest_prefix(V, q):
    m = 0
    while m < len(q) and occurs(V, q[:m + 1]):
        m += 1
    return m


def insert_match(V, s, st):
    """Token for the occurring string `s`: existing, or inserted at its unique
    minimal host by split + join (insert plan revision 3)."""
    if s in V:
        return V
    hosts = minimal_hosts(V, s)
    assert len(hosts) == 1, ('G5: minimal host not unique', s, hosts)
    R, o = hosts[0]
    st['inserted'] += 1
    st['inserted into the input root'] += R == st['root']
    return join(V, split(V, R, o, o + len(s)), R)


def check_root(V, R, st, log):
    """G2 (complete maximal composition) and G4 for the root after an edit."""
    want = brute_cover(R, [k for k in V if k != R])
    if V[R] != want:
        st['root edit not complete (G2)'] += 1
        log.append(f'    G2 {R}: {fmt(V[R])} != {fmt(want)}')
    for (a, x), (b, y) in zip(V[R], V[R][1:]):
        if b < a + len(x) and R[b:a + len(x)] not in V:
            st['root overlap not a token (G4)'] += 1
            log.append(f'    G4 {R}: {x}@{a} {y}@{b}')


class Root:
    def __init__(self):
        self.token, self.owned = None, False

    def rename(self, V, new, comp):
        if self.owned:
            del V[self.token]
        assert new not in V, ('G1: root content exists', new)
        V[new] = comp
        self.token, self.owned = new, True

    def append(self, V, t, st, log):
        R = self.token
        if R is None:
            self.token, self.owned = t, False
        elif R + t in V:
            st['adopt [root, t]'] += 1
            self.token, self.owned = R + t, False
        elif self.owned and t != R:
            st['append in place'] += 1
            self.rename(V, R + t, V[R] + [(len(R), t)])
        else:
            st['new root [root, t]'] += 1
            self.owned = False
            self.rename(V, R + t, [(0, R), (len(R), t)])
        log.append(f'  append {t}: root {self.token} = {fmt(V[self.token]) if V[self.token] else "atom"}')

    def close(self, V, P, E, st, log):
        R = self.token
        o = len(R) - len(P)
        new = R[:o] + E
        if new in V:
            st['closure is a token'] += 1
            self.token, self.owned = new, False
        elif self.owned and E != R:
            st['closure in place'] += 1
            self.rename(V, new, maximal(V[R] + [(o, E)]))
        else:
            # Legacy `closure::prepare`: a self-expansion (E == root) makes
            # the root a child, never an in-place edit (ClosurePolicy::SingleStep).
            st['closure new root' + (' (self-expansion)' if E == R else '')] += 1
            self.owned = False
            self.rename(V, new, maximal([(0, R), (o, E)]))
        log.append(f'  expand {P} -> {E}@{o}: root {self.token} = {fmt(V[self.token])}')


def postfixes(V, R):
    """Postfix tokens of `R`, largest first: the chain of last children."""
    v = R
    while V[v]:
        o, v = V[v][-1]
        yield v


def read(V, I, st, log):
    V = {k: list(c) for k, c in V.items()}
    root, i = Root(), 0
    st['root'] = None
    while i < len(I):
        j = i
        while j < len(I) and I[j] not in V:
            V[I[j]] = []
            j += 1
        for a in I[i:j]:
            root.append(V, a, st, log)
            st['root'] = root.token
        i = j
        while j < len(I) and I[j] in V:
            j += 1
        run, cursor = I[i:j], 0
        while cursor < len(run):
            rest, step = run[cursor:], None
            if cursor > 0:
                for P in postfixes(V, root.token):
                    if len(P) > cursor and SEAM_BOUND:
                        continue
                    st['expansion probes'] += 1
                    m = longest_prefix(V, P + rest)
                    if m > len(P):
                        E = (P + rest)[:m]
                        V = insert_match(V, E, st)
                        root.close(V, P, E, st, log)
                        step = m - len(P)
                        break
            if step is None:
                st['forward searches'] += 1
                m = longest_prefix(V, rest)
                t = rest[:m]
                V = insert_match(V, t, st)
                root.append(V, t, st, log)
                step = m
            st['root'] = root.token
            if root.owned:
                check_root(V, root.token, st, log)
            cursor += step
        i = j
    assert root.token == I, ('root is not the input', root.token, I)
    return V


def classify(got, want):
    extra = set(got) - set(want)
    missing = set(want) - set(got)
    if missing:
        return 'missing tokens'
    if extra:
        return 'only extra tokens (provisional retention)'
    return 'compositions differ'


def gate():
    st, bad = Counter(), []
    for name, inputs in scenarios():
        V = {}
        for k, I in enumerate(inputs):
            log = []
            try:
                V = read(V, I, st, log)
            except (AssertionError, KeyError) as x:
                st['error'] += 1
                bad.append((name, inputs[:k + 1], repr(x)))
                break
            want = graph(inputs[:k + 1])
            if V == want:
                st['reads equal to the oracle'] += 1
                continue
            kind = classify(V, want)
            st['mismatch: ' + kind] += 1
            bad.append((name, inputs[:k + 1], kind, sorted(set(V) - set(want), key=len),
                        sorted(set(want) - set(V), key=len), graph_diff(V, want, 2)))
            V = want  # continue the scenario from the oracle graph
        st['scenarios'] += 1
    st.pop('root', None)
    for k, v in sorted(st.items()):
        print(f'{k}: {v}')
    for b in bad[:12]:
        print(b)


def trace(inputs):
    V, st = {}, Counter()
    for k, I in enumerate(inputs):
        log = []
        V = read(V, I, st, log)
        print(f'read {I!r}')
        print('\n'.join(log))
        want = graph(inputs[:k + 1])
        print('  equals oracle:', V == want, graph_diff(V, want))


if __name__ == '__main__':
    args = sys.argv[1:]
    if args[:1] == ['--no-seam-bound']:
        SEAM_BOUND, args = False, args[1:]
    trace(args) if args else gate()
