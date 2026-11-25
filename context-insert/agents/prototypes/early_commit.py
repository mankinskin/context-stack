"""Dissolution plan task D1b (§3.4, §4.6, Appendix A): root right-maximality and the
early-commit rule, on the reader prototype with eager `settle`
(`read_legacy_port.read` + `dissolve.settle`).

After every settled step of every read:

- RM (§2.8): `root + next atom` occurs in no vertex. With L1 this makes the
  probe of a postfix whose only parent occurrence is the root a rejected
  probe, for roots with and without parents.
- Early-commit rule (§5.1), by definition: a staged composite `t != root`
  is *safe* if its occurrences in the committed inputs and the consumed
  prefix have >= 2 distinct left contexts and >= 2 distinct *fixed* right
  contexts (the occurrence ending at the cursor has none; input starts and
  ends are distinct contexts).
  - off the root's postfix chain, every settled staged composite is safe;
  - on the chain, safe iff the parent-occurrence walk (`fixed_right`)
    finds >= 2 fixed right contexts;
  - the early set: safe tokens whose staged children are all in the early
    set (downward closure), accumulated over the read. No early-committed
    token is dissolved later.
- Base view of a physical early commit: the vertex set `committed + early`
  with brute-force compositions; G4 / G5 (unique minimal host) and the
  number of early tokens with < 2 parent occurrences in it (G6).

Usage:
    python3 early_commit.py              # scenarios and random corpora
    python3 early_commit.py abc abcabca  # one corpus, with a per-step log
"""
import itertools
import random
import sys
from collections import Counter

from common import brute_cover, graph, minimal_hosts, non_token_substrings, scenarios
from dissolve import parents, settle
from read_legacy_port import occurs, postfixes, read


def contexts(texts, t):
    """Left contexts and fixed right contexts of `t` in `texts`
    (`[(string, finished)]`)."""
    left, right = set(), set()
    for k, (s, finished) in enumerate(texts):
        for x in range(len(s) - len(t) + 1):
            if s[x:x + len(t)] != t:
                continue
            left.add(s[x - 1] if x else ('^', k))
            e = x + len(t)
            if e < len(s):
                right.add(s[e])
            elif finished:
                right.add(('$', k))
    return left, right


def fixed_right(V, t, root, inputs, seen=None):
    """Fixed right contexts of `t`, walking parent occurrences: an occurrence
    not at the end of its parent reads the next atom there; one at the end
    inherits the contexts of the parent's occurrences. The root's own end is
    the cursor (no context); an input's own end is one. `inputs` are the
    committed vertices known to be inputs (the implementation knows only the
    parentless ones)."""
    out = set()
    for u, o in parents(V, t):
        e = o + len(t)
        if e < len(u):
            out.add(u[e])
            continue
        if u in inputs:
            out.add(('$', u))
        out |= fixed_right(V, u, root, inputs)
        if len(out) >= 2:
            break
    return out


def base_view(V, committed, early):
    """Brute-force graph of the vertex set `committed + early`; returns the
    list of G4 / G5 violations and the early tokens with p < 2."""
    keys = [v for v in V if v in committed or v in early or len(v) == 1]
    B = {v: (brute_cover(v, keys) if len(v) > 1 else []) for v in keys}
    bad = []
    for N in non_token_substrings(B):
        if len(minimal_hosts(B, N)) != 1:
            bad.append(('G5', N))
    for K, cs in B.items():
        for (a, x), (b, y) in zip(cs, cs[1:]):
            if b < a + len(x) and K[b:a + len(x)] not in B:
                bad.append(('G4', K, x, y))
    low = [x for x in early if len(parents(B, x)) < 2]
    return bad, low


def check_read(prev, I, V, st, bad, log=None, base=False):
    """Read `I` with settle and the checks above; returns the graph."""
    early = set()
    finished = [(s, True) for s in prev]
    inputs = set(prev)

    def after_step(V, committed, root, consumed):
        st['steps'] += 1
        staged_before = [v for v in V if v not in committed]
        root_has_parents = bool(parents(V, root))
        done = settle(V, staged_before, root)
        st['dissolved'] += done['dissolved']
        st['dissolved into the root'] += done['dissolved into the root']
        if root_has_parents:
            st['steps with a root with parents'] += 1
            st['dissolved into a root with parents'] += done['dissolved into the root']
        lost = early - set(V)
        if lost:
            st['early-committed tokens dissolved later'] += len(lost)
            bad.append(('early lost', prev, I[:consumed], sorted(lost)))
            early.difference_update(lost)
        # RM: root + next atom occurs nowhere (next atom already known).
        if consumed < len(I) and I[consumed] in V:
            st['RM checks'] += 1
            if occurs(V, root + I[consumed]):
                st['RM violations'] += 1
                bad.append(('RM', prev, I[:consumed], root, I[consumed]))
        texts = finished + [(I[:consumed], False)]
        chain = set(postfixes(V, root)) if V[root] else set()
        staged = sorted((v for v in V if v not in committed and V[v] and v != root), key=len)
        safe = set()
        for t in staged:
            left, right = contexts(texts, t)
            is_safe = len(left) >= 2 and len(right) >= 2
            local = fixed_right(V, t, root, inputs)
            local_parentless = fixed_right(V, t, root, {v for v in inputs if not parents(V, v)})
            if t in chain:
                st['chain tokens'] += 1
                st['chain tokens safe'] += is_safe
                if (len(local) >= 2) != is_safe:
                    st['chain: walk differs from definition'] += 1
                    bad.append(('walk', prev, I[:consumed], t, local, right))
                if is_safe and len(local_parentless) < 2:
                    st['chain: safe but missed without input marks'] += 1
            else:
                st['off-chain tokens'] += 1
                if not is_safe:
                    st['off-chain tokens not safe'] += 1
                    bad.append(('off-chain unsafe', prev, I[:consumed], t, left, right))
            if is_safe:
                safe.add(t)
        before = len(early)
        for t in staged:  # children first (by width)
            if t in early:
                continue
            if t in safe and all(c in committed or c in early for _, c in V[t] if V[c]):
                early.add(t)
            elif t in safe:
                st['safe tokens blocked by closure (step)'] += 1
        st['early commits'] += len(early) - before
        if base and early:
            violations, low = base_view(V, committed, early)
            st['base views checked'] += 1
            st['base views violating G4/G5'] += bool(violations)
            st['base views with early tokens p < 2'] += bool(low)
            if violations:
                bad.append(('base view', prev, I[:consumed], sorted(early), violations[:3]))
        if log is not None:
            log.append(f'  {I[:consumed]}: root {root}, chain {sorted(chain, key=len)}, '
                       f'safe {sorted(safe, key=len)}, early {sorted(early, key=len)}')

    V = read(V, I, Counter(), [], after_step)
    st['reads'] += 1
    want = graph(prev + [I])
    if V != want:
        st['reads not equal to the oracle'] += 1
        bad.append(('final', prev, I))
    missing = early - set(want)
    st['early tokens not in the final oracle'] += len(missing)
    return V


def run(corpora, st, bad, base=False):
    for inputs in corpora:
        V = {}
        for k, I in enumerate(inputs):
            V = check_read(list(inputs[:k]), I, V, st, bad, base=base)


def gate():
    st, bad = Counter(), []
    run([inputs for _, inputs in scenarios()], st, bad, base=True)
    rng = random.Random(7)
    corpora = [[''.join(w)] for n in range(2, 11) for w in itertools.product('ab', repeat=n)]
    corpora += [[''.join(rng.choice('abc') for _ in range(rng.randint(2, 9)))
                 for _ in range(rng.randint(1, 3))] for _ in range(1500)]
    run(corpora, st, bad, base=False)
    for k, v in sorted(st.items()):
        print(f'{k}: {v}')
    for b in bad[:12]:
        print(b)


def trace(inputs):
    st, bad, V = Counter(), [], {}
    for k, I in enumerate(inputs):
        log = []
        V = check_read(list(inputs[:k]), I, V, st, bad, log, base=True)
        print(f'read {I!r}')
        print('\n'.join(log))
    for k, v in sorted(st.items()):
        print(f'{k}: {v}')
    for b in bad:
        print(b)


if __name__ == '__main__':
    trace(sys.argv[1:]) if sys.argv[1:] else gate()
