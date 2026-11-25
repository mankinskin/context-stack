"""Dissolution of provisional tokens (dissolution plan task D0): the reader
prototype `read_legacy_port.py` with an eager `settle` after every root
commit.

- staged: vertices created by the running read (not in the graph when the
  read started). Committed vertices and atoms are never dissolved.
- `settle(V, staged, root)`: while a staged composite `t != root` has at most
  one parent occurrence: none -> discard `t`; one `(U, o)` -> dissolve:
  `comp(U) := maximal(comp(U) - t@o + (comp(t) + o))`, remove `t`.
  `U` may be committed (only its composition changes).

The gate reads every fixture scenario input by input and checks after every
step (after `settle`):

- the graph equals `oracle(C + [I[:consumed]])` (prefix as input),
- G6 on staged vertices (every staged composite but the root has >= 2
  parent occurrences),
- `settle` largest-first and smallest-first give the same graph,
- committed vertices are kept and every vertex's composition spells its
  string and is complete (G2, brute force);

and after every input: the step decisions `(kind, width, P, E, root)`
equal those of the reader without `settle` (redundancy lemma, plan §2.4),
the graph equals `oracle(C + [I])` and satisfies G6
(every composite that is not an input has >= 2 parent occurrences). It also
checks G6 on the oracle graph of every prefix of every scenario's inputs.

Usage:
    python3 dissolve.py                  # gate (also: read_legacy_port.py --dissolve)
    python3 dissolve.py abc abcabca      # trace reading these inputs with settle
"""
import sys
from collections import Counter

from common import brute_cover, fmt, graph, graph_diff, label, maximal, scenarios
from read_legacy_port import read


def parents(V, t):
    """Parent occurrences `(U, o)` of `t` over all vertices of `V`."""
    return [(u, o) for u, comp in V.items() for o, c in comp if c == t]


def settle(V, staged, root, largest=True, log=None):
    """Dissolve or discard staged composites (except `root`) with <= 1 parent
    occurrence until none is left; edits `V` in place. Returns the count of
    `(dissolved, discarded)`."""
    done = Counter()
    while True:
        cand = [t for t in staged if t in V and t != root and V[t] and len(parents(V, t)) <= 1]
        if not cand:
            return done
        t = (max if largest else min)(cand, key=lambda t: (len(t), t))
        ps = parents(V, t)
        for u, o in ps:
            V[u] = maximal([x for x in V[u] if x != (o, t)] + [(o + a, c) for a, c in V[t]])
            if log is not None:
                log.append(f'    dissolve {t} into {u}@{o}: {u} = {fmt(V[u])}')
        if not ps and log is not None:
            log.append(f'    discard {t}')
        done['dissolved' if ps else 'discarded'] += 1
        done['dissolved into the root'] += any(u == root for u, _ in ps)
        del V[t]


def g6(V, inputs, exempt=()):
    """Composites that are no input (and not exempt) with < 2 parent occurrences."""
    return [v for v in V if V[v] and v not in inputs and v not in exempt
            and len(parents(V, v)) < 2]


def checker(prev, I, st, bad, log=None):
    """`after_step` for `read`: settle, then the per-step checks."""
    def after_step(V, committed, root, consumed):
        assert root == I[:consumed], ('root is not the consumed prefix', root, I[:consumed])
        st['steps'] += 1
        staged = [v for v in V if v not in committed]
        other = {k: list(c) for k, c in V.items()}
        st.update(settle(V, staged, root, True, log))
        settle(other, staged, root, False)
        if other != V:
            st['order dependent settles'] += 1
            bad.append(('order', prev, I[:consumed], graph_diff(V, other, 2)))
        lost = [c for c in committed if c not in V]
        if lost:
            st['steps losing committed vertices'] += 1
            bad.append(('committed lost', prev, I[:consumed], lost))
        for v, comp in V.items():
            if comp and (label(V, comp) != v or comp != brute_cover(v, [k for k in V if k != v])):
                st['vertex not spelling its string or not complete (G2)'] += 1
                bad.append(('G2', prev, I[:consumed], v, comp))
        viol = [v for v in g6(V, (), committed) if v != root]
        if viol:
            st['steps with staged G6 violations'] += 1
            bad.append(('G6 staged', prev, I[:consumed], viol))
        # Atoms of the segment are created before they are consumed.
        want = graph(prev + [I[:consumed]])
        got = {v: c for v, c in V.items() if c or v in want}
        if got == want:
            st['steps equal to the prefix oracle'] += 1
        else:
            st['steps not equal to the prefix oracle'] += 1
            bad.append(('prefix', prev, I[:consumed], sorted(set(got) - set(want), key=len),
                        sorted(set(want) - set(got), key=len), graph_diff(got, want, 3)))
        if log is not None:
            log.append(f'  step {I[:consumed]}: equals prefix oracle: {got == want}')
    return after_step


def snapshot(hook, snaps):
    """`after_step` that runs `hook` and records the graph after the step."""
    def after_step(V, committed, root, consumed):
        if hook:
            hook(V, committed, root, consumed)
        snaps.append({k: list(c) for k, c in V.items()})
    return after_step


def compare_decisions(name, prev, I, V0, plain, settled, st, bad):
    """Redundancy lemma (dissolution plan §3.4): the step decisions
    `(kind, width, P, E, root)` with and without `settle` are equal. On a
    divergence, record both decisions, the probed postfixes and the tokens
    dissolved before the step with their parent occurrences in the plain graph."""
    st['reads compared'] += 1
    (pt, ps), (dt, ds) = plain, settled
    if [p for _, p in pt] != [p for _, p in dt]:
        st['reads with different probed postfixes'] += 1
        n = next(n for n, (a, b) in enumerate(zip(pt, dt)) if a[1] != b[1])
        bad.append(('probes', name, prev, I, f'step {n}', 'plain', pt[n], 'settled', dt[n]))
    if [d for d, _ in pt] == [d for d, _ in dt]:
        st['reads with identical decisions'] += 1
        return
    st['reads with diverging decisions'] += 1
    n = next(n for n, (a, b) in enumerate(zip(pt, dt)) if a[0] != b[0])
    Gp, Gd = (ps[n - 1], ds[n - 1]) if n else (V0, V0)
    root = pt[n - 1][0][4] if n else None
    gone = [(t, Gp[t], parents(Gp, t)) for t in sorted(set(Gp) - set(Gd), key=len)]
    in_root = any(u == root for _, _, ps_ in gone for u, _ in ps_)
    st['diverging reads: dissolved token had its parent occurrence in the root'] += in_root
    bad.append(('decisions', name, prev, I, f'step {n}',
                'plain', pt[n], 'settled', dt[n], 'root before', root,
                'plain root comp', Gp.get(root), 'settled root comp', Gd.get(root),
                'dissolved', gone, 'final equals oracle', ds[-1] == graph(prev + [I])))


def oracle_g6(st):
    """G6 on the oracle graph of every prefix of every scenario's inputs."""
    for _, inputs in scenarios():
        for k in range(1, len(inputs) + 1):
            st['oracle graphs'] += 1
            viol = g6(graph(inputs[:k]), inputs[:k])
            st['oracle graphs violating G6'] += bool(viol)


def gate():
    st, bad = Counter(), []
    for name, inputs in scenarios():
        V = {}
        for k, I in enumerate(inputs):
            try:
                plain, settled = ([], []), ([], [])
                read(V, I, Counter(), [], snapshot(None, plain[1]), plain[0])
                check = checker(inputs[:k], I, st, bad)
                V0, V = V, read(V, I, Counter(), [], snapshot(check, settled[1]), settled[0])
            except (AssertionError, KeyError) as x:
                st['error'] += 1
                bad.append(('error', name, inputs[:k + 1], repr(x)))
                break
            compare_decisions(name, inputs[:k], I, V0, plain, settled, st, bad)
            want = graph(inputs[:k + 1])
            st['reads'] += 1
            st['reads violating G6'] += bool(g6(V, inputs[:k + 1]))
            if V == want:
                st['reads equal to the oracle'] += 1
                continue
            st['reads not equal to the oracle'] += 1
            bad.append(('final', name, inputs[:k + 1], graph_diff(V, want, 2)))
            V = want
        st['scenarios'] += 1
    oracle_g6(st)
    for k, v in sorted(st.items()):
        print(f'{k}: {v}')
    for b in bad[:12]:
        print(b)


def trace(inputs):
    V, st, bad = {}, Counter(), []
    for k, I in enumerate(inputs):
        log = []
        V = read(V, I, Counter(), log, checker(inputs[:k], I, st, bad, log))
        print(f'read {I!r}')
        print('\n'.join(log))
        want = graph(inputs[:k + 1])
        print('  equals oracle:', V == want, graph_diff(V, want))
    for b in bad:
        print(b)


def main(args):
    trace(args) if args else gate()


if __name__ == '__main__':
    main(sys.argv[1:])
