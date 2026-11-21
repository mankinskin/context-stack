"""Fixture-wide checks of the facts the insert plan relies on (Part B), on the
checked-in v3 fixture graphs:

- F3 unique minimal host: every substring (2+ atoms) of a vertex string that
  is not itself a vertex has exactly one occurrence `(vertex, offset)` that
  lies in no child of that vertex;
- I2 overlap closure: for every two overlapping occurrences in a
  composition, the overlapping string is a vertex.

Usage:
    python3 invariants.py
"""
from collections import Counter

from common import fixture_graph, minimal_hosts, non_token_substrings, scenarios


def main():
    c = Counter()
    for name, _ in scenarios():
        V = fixture_graph(name)
        for N in non_token_substrings(V):
            hosts = minimal_hosts(V, N)
            c['F3 substrings'] += 1
            if len(hosts) != 1:
                c['F3 exceptions'] += 1
                print('F3 exception', name, repr(N), hosts)
        for K, cs in V.items():
            for i in range(len(cs)):
                for j in range(i + 1, len(cs)):
                    (a, x), (b, y) = cs[i], cs[j]
                    lo, hi = max(a, b), min(a + len(x), b + len(y))
                    if lo < hi:
                        c['I2 overlapping pairs'] += 1
                        if K[lo:hi] not in V:
                            c['I2 exceptions'] += 1
                            print('I2 exception', name, K, x, y, K[lo:hi])
    for k in ('F3 substrings', 'F3 exceptions', 'I2 overlapping pairs', 'I2 exceptions'):
        print(f'{k}: {c[k]}')


if __name__ == '__main__':
    main()
