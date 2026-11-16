# Notation and roles

Positions are zero-based atom offsets. `I[x, y)` includes `x` and excludes `y`.
`+` means concatenation.

| Symbol | Meaning |
| --- | --- |
| `I` | Entire input. |
| `G` | Graph state at the time of a search. |
| `b`, `e` | Current block boundaries: `I[b, e)`. |
| `c` | Commit cursor, with `b <= c <= e`. |
| `R` | Accumulation root, representing `I[0, c)`. |
| `U` | Remaining block: `I[c, e)`. |
| `P` | Canonical shared overlap token, a proper postfix of `R`. |
| `E` | Accepted expansion beginning with `P` and extending into `U`. |
| `X1` | Last child of an editable root. |
| `X` | Closure scope: `X1` for an editable root, otherwise `R`. |
| `L` | Left complement token: `X = L + P`, represented at offset 0 of `X`. |
| `T` | Right complement token: `E = P + T`, represented at offset `\|P\|` of `E`. |
| `C` | Closed token: `[X,T] = [L,E]` for a single-step chain. |
| `Si` | Widest-postfix chain of `X`: `P = S0 < ... < Sk = X`, `Si = Li + S(i-1)`. |
| `Ci` | Nested closure `[Si,T] = [Li,C(i-1)]` with `C0 = E` and `C = Ck`. |

The **commit seam** is `I[b, c)`: the entire committed part of the block.
It always starts at `b` and is a postfix of `R`.

```text
input:  [ earlier input ][ committed block ][ remaining block ]
        0               b                   c                 e
root:   [-----------------------------------)
seam:                   [-------------------)
block:                  [-------------------------------------)
```

An **anchor** supplies postfixes for overlap search. The root can serve as a
**seam anchor** while remaining the sink's accumulation destination.

The **postfix chain** follows the widest last child among each traversed
token's decompositions. Its steps form a connected occurrence path from `R`.
Eligible postfixes satisfy `|P| <= c-b`; wider postfixes are traversed but not
searched. The query is `P + U`. A **crossing match** has `|E| > |P|` and consumes
`|E|-|P|` atoms. There is no enumeration of seam atom positions.

A **token** is a graph vertex; an **occurrence** is an interval containing its
content. A **largest match** at a position `q` is the longest prefix of `I[q, e)` found in
`G`, either as a token or a subspan of a larger token. Its content and length
are unique, although multiple graph paths may represent it.

Larger tokens represent subtokens transitively through their decompositions.
Overlapping occurrences may require alternative decompositions; flattened
content alone does not establish that the graph represents every occurrence.
