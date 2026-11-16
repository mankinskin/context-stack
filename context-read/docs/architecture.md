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

With `X = L + P` and `E = P + T`, close as:

```text
C = [X,T...] = [L...,E]
```

`L` and `T` are sibling patterns. Prefer the unique existing decomposition with
the required boundary; otherwise collect siblings along connected paths.
Do not allocate complement-only tokens. Reuse an existing canonical `C`.

If `P` is the entire editable last child and `E` is not the root, replace that
child directly with `E`, without constructing `C`.

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
