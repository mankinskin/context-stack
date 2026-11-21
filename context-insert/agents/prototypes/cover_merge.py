"""Cover projection and merge with seam discovery (plan Part B "Merge", task
3.5), on the checked-in fixture graphs.

1. Projection: `cover(R, [s, e))` recursing only into children meeting the
   span equals the brute-force maximal occurrences of `R[s:e]`, for every
   proper span (2+ atoms) of every vertex.
2. Merge: for every vertex `V` (3+ atoms) and every split into a left side
   `[0, wl)` and a right side `[r0, |V|)` with `0 < r0 <= wl < |V|`, on the
   graph without `V`: left cover ∪ right cover shifted by `r0` ∪ seam tokens
   found by scanning starts `x = r0 - 1 .. 0` (stop at the first `x` whose
   longest occurring prefix ends inside the left side, F4) equals `V`'s
   composition. Reports the seam searches against scanning every start.
3. `--reinsert` (revision 1, superseded): remove a token that occurs in one
   host composition and re-add it at that host. Removing a token gives graphs
   the oracle cannot produce; kept for reference only.

Usage:
    python3 cover_merge.py [--reinsert]
"""
import sys
from collections import Counter

from common import brute_cover, fixture_graph, maximal, scenarios


def main(reinsert):
    stats = Counter({'projection failures': 0, 'merge failures': 0})
    if reinsert:
        stats['reinsert failures'] = 0
    for name, _ in scenarios():
        V = fixture_graph(name)
        keys = set(V)

        def cover(R, s, e):
            if s == 0 and e == len(R):
                return [(0, R)]
            res = []
            for a, c in V[R]:
                lo, hi = max(s, a), min(e, a + len(c))
                if lo < hi:
                    res += [(lo + x - s, t) for x, t in cover(c, lo - a, hi - a)]
            return maximal(res)

        for R in keys:
            if len(R) < 2:
                continue
            for s in range(len(R)):
                for e in range(s + 2, len(R) + 1):
                    if (s, e) == (0, len(R)):
                        continue
                    sub = R[s:e]
                    want = [(0, sub)] if sub in keys else brute_cover(sub, keys)
                    stats['projection cases'] += 1
                    stats['projection failures'] += cover(R, s, e) != want

        if reinsert:
            for T in keys:
                if len(T) < 2:
                    continue
                hosts = [(K, o) for K, c in V.items() for o, x in c if x == T]
                if len(hosts) != 1:
                    continue
                rk = keys - {T}
                Rh, o = hosts[0]
                new = maximal(brute_cover(Rh, rk) + [(o, T)])
                new = [(x, t) for x, t in new if not (o <= x and x + len(t) <= o + len(T)) or t == T]
                ok = sorted(new) == sorted(V[Rh]) and brute_cover(T, rk) == V[T]
                stats['reinsert ok' if ok else 'reinsert failures'] += 1

        for Vk in keys:
            n = len(Vk)
            if n < 3:
                continue
            rk = keys - {Vk}
            strs = list(rk)

            def occurs(q):
                return any(q in k for k in strs)

            for r0 in range(1, n):
                for wl in range(r0, n):
                    L, Rt = Vk[:wl], Vk[r0:]
                    lc = [(0, L)] if L in rk else brute_cover(L, rk)
                    rc = [(0, Rt)] if Rt in rk else brute_cover(Rt, rk)
                    seam = []
                    for x in range(r0 - 1, -1, -1):
                        stats['seam searches'] += 1
                        m = x + max(j for j in range(1, n - x + 1) if occurs(Vk[x:x + j]))
                        if m <= wl:
                            break
                        S = max((t for t in rk if Vk.startswith(t, x) and x + len(t) <= m), key=len)
                        if x + len(S) > wl:
                            seam.append((x, S))
                    stats['searches when scanning every start'] += r0
                    got = maximal(lc + [(r0 + x, t) for x, t in rc] + seam)
                    stats['merge cases'] += 1
                    if got != V[Vk]:
                        stats['merge failures'] += 1
                        print('merge failure', name, Vk, r0, wl, got, V[Vk])
    for k, v in sorted(stats.items()):
        print(f'{k}: {v}')


if __name__ == '__main__':
    if sys.argv[1:] not in ([], ['--reinsert']):
        sys.exit(__doc__)
    main(sys.argv[1:] == ['--reinsert'])
