"""Find deep insert examples: a query that is not a token, has a unique
minimal host, and none of whose proper prefixes (2+ atoms) or postfixes
(2+ atoms) is a token, so both response paths are long.

Modes:
    python3 find_examples.py --fixtures [N]
        rank the queries of the checked-in fixtures by path depth (top N, 25)
    python3 find_examples.py --random SEED SECONDS [--metric paths|levels]
        sample small random corpora (1-3 strings of 6-14 atoms over 2-4 letters)
        for SECONDS and print the best 8 as
        (score, total corpus length, corpus, (query, root, offset))

Metric `paths` scores (min, sum) of the response path lengths; `levels`
scores (min, sum) of the number of distinct intersection widths per side, i.e.
the levels the join must create, then prefers more distinct letters and
shorter queries. Results depend on the seed and the time budget.
"""
import random
import sys
import time

from common import end_path, graph, minimal_hosts, scenarios, start_path


def candidates(V):
    for N in {K[a:b] for K in V for a in range(len(K)) for b in range(a + 3, len(K) + 1)} - set(V):
        if any(N[:j] in V for j in range(2, len(N))) or any(N[j:] in V for j in range(1, len(N) - 1)):
            continue
        hosts = minimal_hosts(V, N)
        if len(hosts) != 1:
            continue
        R, o = hosts[0]
        yield N, R, o, start_path(V, R, o, o + len(N)), end_path(V, R, o, o + len(N))


def levels(path, o, N, side):
    """Distinct widths (2+) of the intersections of `N` with the path nodes."""
    widths, ab = set(), 0
    for _, off, ch in path:
        ab += off
        w = (ab + len(ch) - o) if side == 'start' else (o + len(N) - ab)
        if w > 1 and ch != N:
            widths.add(w)
    return len(widths)


def score(C, metric):
    best = None
    for N, R, o, sp, ep in candidates(graph(C)):
        if metric == 'paths':
            s = (min(len(sp), len(ep)), len(sp) + len(ep), -len(N))
        else:
            ls, le = levels(sp, o, N, 'start'), levels(ep, o, N, 'end')
            s = (min(ls, le), ls + le, len(set(N)), -len(N))
        if best is None or s > best[0]:
            best = (s, N, R, o)
    return best


def rank_fixtures(top):
    res = []
    for name, corpus in scenarios():
        V = graph(corpus)
        for N, R, o, sp, ep in candidates(V):
            res.append((min(len(sp), len(ep)), len(sp) + len(ep), name, N, R, o))
    for r in sorted(res, reverse=True)[:top]:
        print(r)


def sample(seed, seconds, metric):
    random.seed(seed)
    seen, t = [], time.time()
    while time.time() - t < seconds:
        n = random.choice([1, 2, 2, 3])
        alpha = 'abcd'[:random.choice([2, 3, 4] if metric == 'paths' else [3, 3, 4])]
        C = [''.join(random.choice(alpha) for _ in range(random.randint(6, 14))) for _ in range(n)]
        b = score(C, metric)
        if b:
            seen.append((b[0], sum(map(len, C)), C, b[1:]))
    seen.sort(key=lambda x: (x[0], -x[1]), reverse=True)
    for r in seen[:8]:
        print(r)


if __name__ == '__main__':
    a = sys.argv[1:]
    if a[:1] == ['--fixtures'] and len(a) <= 2:
        rank_fixtures(int(a[1]) if len(a) == 2 else 25)
    elif a[:1] == ['--random'] and len(a) in (3, 5) and (len(a) == 3 or a[3] == '--metric'):
        sample(int(a[1]), float(a[2]), a[4] if len(a) == 5 else 'paths')
    else:
        sys.exit(__doc__)
