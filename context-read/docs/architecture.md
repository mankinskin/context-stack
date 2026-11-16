# Algorithm responsibilities

| Component | Responsibility |
| --- | --- |
| `ExpansionCtx` | Search the current graph and prepare one consuming operation. |
| Closure module | Resolve a structural overlap and select its integration scope. |
| `RootManager` | Apply prepared operations with ownership and stale-state checks. |
| `BlockExpansionCtx` | Commit immediately and return the new root to expansion. |

## Root-first loop

The accumulated root is the only overlap anchor. Before each forward step,
follow its proper postfix chain largest-first. Search only postfixes `P` with
`|P| <= c-b`; larger postfixes are traversed to reach eligible descendants.
An empty seam skips overlap search.

Probe `P + I[c,e)` read-only. Accept the first confirmed match `E` wider than
`P`, materialize it, and consume `|E|-|P|` new atoms. Materialization can rewrite
decompositions, so reconnect the path to `P` and recompute root editability.

If no postfix extends, materialize the largest forward match and append it.
Expansion reuses an existing canonical combined root when found, emitting a
root replacement rather than asking the sink to allocate a duplicate.
Every operation is committed before the next search. There is no local anchor,
pending span, seam-position scan or multi-step overlap chain.

## Factored closure

The search anchor, closure scope and accumulation destination have distinct roles.

- For a sink-owned, single-pattern, parentless root, scope `X` is its last child.
- Otherwise `X` is the entire root.
- When the expansion is the root itself, use whole-root scope, even if it was
  editable before the search.

With `X = L + P` and `E = P + T`, close bottom-up along the widest-postfix
chain of `X` (following the widest terminal child at each level):

```text
P = S0 < S1 < ... < Sk = X        Si = Li + S(i-1)
C0 = E
Ci = [Si,T] = [Li,C(i-1)]         C = Ck
```

Every existing postfix token `S` of `X` with `|P| < |S| <= |X|` occurs left
of the expansion's overlap and is therefore closed with it, e.g. `abc=[a,bc]`
and `cd` sharing `c` yield `bcd=[bc,d]=[b,cd]` and `abcd=[abc,d]=[a,bcd]`.
Postfixes not wider than `P` already lie inside `E`. Boundary witnesses may
skip chain tokens (`abc -> c`), so the chain follows the widest terminal
child; `P` must lie on it. Self-expansions close the root in one step.

Each `Ci` is binary. `T` and every `Li` are canonical **tokens**, each
represented inside its smallest scope: `Li` is reachable at offset 0 of `Si`
and `T` at offset `|P|` of `E`, through connected child patterns. `T` is
resolved once; each later step reuses it as the last child of `C(i-1)`. A
complement is never scoped to a wider range that includes the root prefix.

Resolve each complement in order:

1. Reuse a connected child occurrence of the required width and offset,
   descending through all child patterns (a single token is used directly).
2. If no connected occurrence exists but a canonical token with that content
   does, panic: the occurrence path is missing, which is an insertion bug.
3. Otherwise materialize it with scoped insertion
   (`context_insert::insert::scoped`), which splits only the scope and its
   descendants and rewrites their decompositions so the complement is
   connected without duplicating boundaries.

Sibling patterns must not be used as complements: `[X,T...] = [L...,E]` would
repeat sequences inside a child of the other decomposition, e.g.
`visu=[vis,u]=[v,i,su]` repeats `[v,i]` inside `vis`. The correct closure is
`vi=[v,i]`, `vis=[vi,s]`, `visu=[vis,u]=[vi,su]`.

Preparation is read-only: it selects the scope, records the intended tail or
root integration, and collects the chain tokens. Structural application
returns a token independently of integration; the integration operation then
wraps it in the appropriate sink span. For a nonempty chain, application
resolves `T`, then per step resolves `Li`, re-derives the `Si -> S(i-1)` and
`C(i-1) -> S(i-1)` paths
(insertion may rewrite decompositions), validates the complement occurrences
and creates `Ci` or adds its missing decompositions to an existing canonical
`Ci` (content lookup of `[Si] + atoms(T)`), whose decompositions may equal but
must not share planned boundaries `{|Si|}` or `{|Li|}`. Only the final `C` is
committed to the sink. The expansion step then asserts that the root is
unchanged and, for tail replacement, still editable.

Scoped insertion and closure share `context_trace::boundary_target` to validate
connected prefix/postfix paths. Closure additionally checks that the returned
token is the expected overlap token.

If `P` is the entire editable last child and `E` is not the root, replace that
child directly with `E`: the same structural executor receives an empty chain
and returns `E` without resolving complements or mutating the graph.

## Sink protocol

| Operation | Effect |
| --- | --- |
| `Append(token)` | Adopt, grow an owned container, or allocate `[root,token]`. |
| `ReplaceTail { previous, old, token }` | Replace an editable root's last child. |
| `ReplaceRoot { previous, token }` | Adopt a larger result without modifying the previous root. |

Tail replacement validates the current root, expected child, ownership,
single pattern, no parents, width growth and absence of self-reference/cycles.
It updates root width and child-parent metadata. The sink never searches.

`CommitReceipt` contains only the new root. Expansion borrows the sink when
preparing a step to check ownership against the current graph, rather than
caching an editability flag.

## Limits and validation

The algorithm relies on the graph invariants in [graph changes](graph-changes.md).
Tests validate canonical content, represented occurrences and distinct boundaries
for the focused closure and repetition cases.

Closure planning is read-only after accepted expansion materialization.
Invariant violations panic; materialization is not transactional and there is
no rollback. Existing insertion/search failures remain separate work, recorded
in the [task list](overlap-refactor-tasks.md).
