"""Show how the oracle graph changes when inputs are appended to a corpus.

Usage:
    python3 oracle_diff.py CORPUS... + INPUT...
    python3 oracle_diff.py acabcabcab bcabcb + abcabca

Prints the graph of CORPUS, then the vertices created, removed and recomposed
in the graph of CORPUS + INPUT. Use it to derive expectations for new
examples; pass only the minimal larger strings, the oracle derives the
repeated children.
"""
import sys

from common import fmt, graph


def main(argv):
    if '+' not in argv or argv.index('+') == 0 or argv[-1] == '+':
        sys.exit(__doc__)
    i = argv.index('+')
    before, after = argv[:i], argv[:i] + argv[i + 1:]
    G0, G1 = graph(before), graph(after)
    print(f'graph of {before}:')
    for v in sorted(G0, key=lambda v: (len(v), v)):
        if G0[v]:
            print(f'    {v} = {fmt(G0[v])}')
    print(f'changes for {after}:')
    for v in sorted(set(G0) | set(G1), key=lambda v: (len(v), v)):
        if v not in G1:
            print(f'    removed {v}')
        elif v not in G0:
            print(f'    new     {v} = {fmt(G1[v])}')
        elif G0[v] != G1[v]:
            print(f'    changed {v}: {fmt(G0[v])}  ->  {fmt(G1[v])}')


if __name__ == '__main__':
    main(sys.argv[1:])
