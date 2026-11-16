# Graph changes and eligible postfixes

Using the [notation](notation.md), let `C(G)` contain every nonempty substring
of every token in graph `G`.

## When can a largest match change?

If a change preserves content, `C(G_before)` is a subset of `C(G_after)`.
For a fixed query, the largest content match cannot shrink. It grows exactly
when the previous result can now be extended by at least the next query atom.

- Extending an owned root in place can add content without allocating a vertex.
- Materializing an existing substring adds no content, but can change token
  identity and graph paths.
- A collapse can introduce content before the sink commits it.

These are content-level properties; actual search and mutation must preserve
the corresponding graph invariants.

## Where does new content come from?

For a simple concatenation `R_new = R_old + T`, assume both sides already
occurred in the previous graph and no other content was added. Every newly
available substring must cross the junction between the two sides.

This narrows the source of a changed result, not automatically its query start.

## Filtering root postfixes

Assume the only added content is contained in updated root `R`, and `U` is
nonempty. A newly available crossing match for root postfix `P` requires
`P` followed by the first atom of `U` to occur inside `R`.

That occurrence must start before `c-|P|`: it fits within the root while the
queried occurrence extends beyond it.

**Whole-seam rejection:** every such sequence ends with the boundary pair
`I[c - 1, c + 1)`. If that pair does not occur inside `R`, no root-induced new
crossing match is possible.

Example: root `aabb`, next atom `a`. The boundary pair is `ba`, absent from
`aabb`, so this root-induced crossing search can be skipped.

These are necessary-condition filters, not implemented optimizations. They
do not rule out previously available matches, changes elsewhere in the graph,
or larger matches ending before the cursor. A full-seam repetition of the
pre-block prefix is not required: shorter seam postfixes can also matter.

## Structural postfix-chain argument

The implementation relies on three invariants:

1. There is one canonical vertex per token content.
2. Every occurrence of that content inside a larger token is reachable by a
   connected child path.
3. No internal top-level boundary is shared by multiple decompositions of a token.

Proper postfix tokens all end at the same position. The widest proper postfix
contains all smaller postfix occurrences, and occurrence completeness makes
them reachable inside it. That widest postfix must be a last child of some
decomposition: a larger terminal ancestor would contradict its maximality.

Therefore traversal repeatedly chooses the widest last child, ending at an
atom. It does not need a queue of alternative suffix branches. For example:

```text
abcde = [a,bcde] = [abc,de]
bcde  = [b,cde]  = [bc,de]
cde   = [c,de]
de    = [d,e]
chain: bcde -> cde -> de -> e
```

A crossing overlap shares one canonical token `P` represented as the anchor
postfix and expansion prefix. Closure uses those occurrence paths rather than
flattening the graph into atom sequences.

These arguments do not repair invalid insertion output. The focused test
validator checks the invariants; insertion violations are tracked separately.
