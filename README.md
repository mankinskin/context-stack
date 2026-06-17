# context-stack

context-stack is the standalone repository for the extracted graph, search, insert, and read crates that are mounted back into `context-engine` as the `context-stack` submodule.

## Repository Map

| Child crate or folder | What it contains | Direct README |
| --- | --- | --- |
| [context-api](context-api/README.md) | API-level entry points for context operations. | [context-api/README.md](context-api/README.md) |
| [context-trace](context-trace/README.md) | Foundational graph structures, traversal, and tracing primitives. | [context-trace/README.md](context-trace/README.md) |
| [context-search](context-search/README.md) | Search and traversal layers built on the trace crate. | [context-search/README.md](context-search/README.md) |
| [context-insert](context-insert/README.md) | Insert and mutation workflows for the graph structures. | [context-insert/README.md](context-insert/README.md) |
| [context-read](context-read/README.md) | Read and expansion workflows on top of the shared graph stack. | [context-read/README.md](context-read/README.md) |
| [deps/petgraph](deps/petgraph/README.md) | Vendored dependency source used by this repository. | [deps/petgraph/README.md](deps/petgraph/README.md) |

Additional folders without repository-level READMEs yet include `context-trace-macros` and `ngrams`; the submodule wiring for the dependency folders is tracked in [.gitmodules](.gitmodules).

## Installable Content

This repository does not publish a root-level executable binary. The root surface is a library workspace made up of the child crates above, and each crate README is the direct entry point for its API and workflow details.

## Workspace Commands

```bash
cargo test --workspace
cargo doc --workspace --open
```

- `cargo test --workspace` is documented in [The Cargo Book: cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html).
- `cargo doc --workspace --open` is documented in [The Cargo Book: cargo doc](https://doc.rust-lang.org/cargo/commands/cargo-doc.html).
- Child crate workflows and crate-specific commands live in [context-api/README.md](context-api/README.md), [context-trace/README.md](context-trace/README.md), [context-search/README.md](context-search/README.md), [context-insert/README.md](context-insert/README.md), and [context-read/README.md](context-read/README.md).
