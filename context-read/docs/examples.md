# Repetition and factored closure

See [notation](notation.md) for symbols and intervals.

## `aaa`

After the second atom commits, `R=aa`, the seam is one `a`, and one `a`
remains. Root postfix `P=a` expands to `E=aa=R`.
Do not replace the root's last child by the root itself. Close at root scope:

```text
aa  = [a,a]
aaa = [aa,a] = [a,aa]
```

## `abc|abcabc`

Commit the next forward `abc`, creating `R=abcabc=[abc,abc]`.
The seam contains the second `abc`; the remaining input is the third.
Root postfix `P=abc` expands to `E=abcabc=R`. Again close at root scope:

```text
abcabcabc = [abcabc,abc] = [abc,abcabc]
```

The larger token represents smaller occurrences transitively; no atom-position
seam scan or materialization of every substring is needed.

## `aabbaabb`

The known block begins after `aab`. Its first `b` cannot expand, so it commits
to the owned root, producing flat `aabb=[a,a,b,b]`. The seam is now one `b`.
That postfix still does not extend into `aabb`; the forward search finds `aabb`.
Appending the root to itself freezes it:

```text
aabbaabb = [aabb,aabb]
```

Only two atoms and two compound vertices remain.

## `xyzabcd`, then `xyzabcde`

With editable `R=[x,y,z,abc]`, expansion `bcd` overlaps at `bc`.
Search uses `R`, but closure uses its entire last child `X=abc`:

```text
abcd = [abc,d] = [a,bcd]
R    = [x,y,z,abcd]
```

An ensuing `cde` overlap at `cd` closes around `X=abcd` with `L=ab`, which is
materialized inside `abcd`'s nested child `abc`:

```text
ab    = [a,b]
abc   = [a,bc] = [ab,c]
abcde = [abcd,e] = [ab,cde]
R     = [x,y,z,abcde]
```

The root prefix is not duplicated across alternatives. The complement `ab` is
a token represented inside its scope, not the sibling run `[a,b]`, which would
repeat content inside `abc`.

## Flat scope: `x` + `vis`, then `su`

With editable `R=[x,vis]`, flat `vis=[v,i,s]` and expansion `E=su=[s,u]`
overlapping at `P=s`, the left complement `vi` is inserted inside `vis`:

```text
vi   = [v,i]
vis  = [vi,s]
visu = [vis,u] = [vi,su]
R    = [x,visu]
```

Collecting siblings instead (`visu=[vis,u]=[v,i,su]`) would repeat `[v,i]`
inside `vis` and leave `vi` unrepresented.

## Nested complements: `x` + `wabc`, then `bcdef`

With `wabc=[w,abc]`, `abc=[a,bc]`, `P=bc` and `E=bcdef=[bcde,f]`,
`bcde=[bcd,e]`, `bcd=[bc,d]`, both complements are nested:

```text
wa      = [w,a]          (inside wabc)
wabc    = [w,abc] = [wa,bc]
def     = [d,e,f]        (inside bcdef)
bcde    = [bcd,e]
bcdef   = [bcde,f] = [bc,def]
wabcdef = [wabc,def] = [wa,bcdef]
```

The interval `de` is only needed to resolve the boundary of `def` inside
`bcde`. Its only evidence parent is `def` and `bcde=[bc,de]` would merely be a
sub-merge proposal, so insertion inlines it instead of publishing a token.

## Preserve nested tokens and canonical complements

If the last child is `wabc=[w,abc]`, close around `wabc`, not nested `abc`.
Otherwise the original `wabc` occurrence could disappear.

When `wa` already exists and `wabc=[w,abc]=[wa,bc]`, reuse its boundary:

```text
X = wabc, P = bc, E = bcd, L = wa, T = d
wabcd = [wabc,d] = [wa,bcd]
```

Do not collect `[w,a]` instead of the canonical complement `wa`; single-token
complements are reused unchanged.
