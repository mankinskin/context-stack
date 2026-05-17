# context-stack

Standalone repository for the extracted context graph crates from `context-engine`.

Repository layout:

- `context-api`
- `context-insert`
- `context-read`
- `context-search`
- `context-trace`
- `context-trace-macros`
- `ngrams`
- `deps/petgraph` (submodule)
- `deps/justlog` (submodule)

This repository is intended to be mounted back into `context-engine` as the
`context-stack` submodule. Its history is rewritten from the source
repository so that `main` reflects the standalone submodule root rather than the
original `crates/*` paths.
