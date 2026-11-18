# Plans Index — ngrams

Plans for the n-gram oracle: labelling semantics, output format and pipeline.

## Active Plans

| Date | File | Status | Summary |
|------|------|--------|---------|
| 2026-11-18 | [20261118_PLAN_POSITIONED_COMPOSITION_ORACLE](20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md) | 📋 ready | Fix order dependence by removing wrapper/gap vertices: retain only roots, atoms and frequency-selected tokens, store positioned compositions (offsets, overlaps allowed), and derive rows/gaps/wrappers as optional owner-local layout. Introduces output format v3 and coordinates downstream fixture consumers. |
| 2026-11-18 | [Positioned oracle implementation checklist](20261118_PLAN_POSITIONED_COMPOSITION_ORACLE_IMPLEMENTATION.md) | 🚧 cutover done | File-by-file migration, v2 compatibility boundary, exact regression cases, fixture migration, and validation gates. |
