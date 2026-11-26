# context-stack

context-stack is the standalone repository for the extracted graph, search, insert, and read crates that are mounted back into `context-engine` as the `context-stack` submodule.

## Repository Map

| Child crate or folder | What it contains | Direct README |
| --- | --- | --- |
| [context-api](context-api/README.md) | Command API of the stack: graph store (`.ctx` files), sessions, read, search views, render, export/import. | [context-api/README.md](context-api/README.md) |
| [tools/cli/context-cli](tools/cli/context-cli/README.md) | `context-cli`: graph commands, interactive shell, client mode, `serve` and `mcp` subcommands. | [tools/cli/context-cli/README.md](tools/cli/context-cli/README.md) |
| [tools/http/context-http](tools/http/context-http/README.md) | HTTP and WebSocket server over `context-api` (library and `context-http` binary). | [tools/http/context-http/README.md](tools/http/context-http/README.md) |
| [tools/mcp/context-mcp](tools/mcp/context-mcp/README.md) | MCP server with typed tools, local or proxying a running server. | [tools/mcp/context-mcp/README.md](tools/mcp/context-mcp/README.md) |
| [context-trace](context-trace/README.md) | Foundation: ids, atoms, logging, the positioned graph store with read traits, navigation core and v3 interchange/fixtures. | [context-trace/README.md](context-trace/README.md) |
| [context-search](context-search/README.md) | Positioned search; owns search paths, trace states and the trace cache. | [context-search/README.md](context-search/README.md) |
| [context-insert](context-insert/README.md) | Positioned insertion; owns every mutation (`GraphEdit`) and the insert fixture lane. | [context-insert/README.md](context-insert/README.md) |
| [context-read](context-read/README.md) | Positioned reader; owns the overlay stage and the read fixture lane. | [context-read/README.md](context-read/README.md) |
| [context-trace-legacy](context-trace-legacy/README.md) | Frozen row model (pattern-row `Hypergraph`, paths, trace cache) extracted from context-trace. | [context-trace-legacy/README.md](context-trace-legacy/README.md) |
| [context-search-legacy](context-search-legacy/README.md) | Frozen row-model search on context-trace-legacy. | [context-search-legacy/README.md](context-search-legacy/README.md) |
| [context-insert-legacy](context-insert-legacy/README.md) | Frozen row-model insertion (split-join, overlap closure). | [context-insert-legacy/README.md](context-insert-legacy/README.md) |
| [context-read-legacy](context-read-legacy/README.md) | Frozen row-model reader and its v2 oracle tests. | [context-read-legacy/README.md](context-read-legacy/README.md) |
| `deps/petgraph/README.md` | Vendored dependency source used by this repository. | `deps/petgraph/README.md` |

Additional folders without repository-level READMEs yet include `context-trace-macros` and `ngrams`; the submodule wiring for the dependency folders is tracked in `.gitmodules`.

## Installable Content

The user-facing executable is `context-cli` (`cargo install --path tools/cli/context-cli`): create graphs from text, search them, render small graphs, serve them over HTTP/WebSocket and MCP. `context-http` and `context-mcp` are also available as standalone binaries. The algorithm crates are libraries; each crate README is the entry point for its API.

```bash
context-cli create notes -f notes/ --recursive --glob '*.md'
context-cli search notes "brown fox" --view occurrences
context-cli serve notes &          # later commands use the running server
context-cli shell notes
```

## Workspace Commands

```bash
cargo test --workspace
cargo doc --workspace --open
```

- `cargo test --workspace` is documented in [The Cargo Book: cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html).
- `cargo doc --workspace --open` is documented in [The Cargo Book: cargo doc](https://doc.rust-lang.org/cargo/commands/cargo-doc.html).
- Child crate workflows and crate-specific commands live in [context-api/README.md](context-api/README.md), [context-trace/README.md](context-trace/README.md), [context-search/README.md](context-search/README.md), [context-insert/README.md](context-insert/README.md), and [context-read/README.md](context-read/README.md).
- Layering: context-trace < context-search < context-insert < context-read; each crate depends only on lower layers (guard test `crates_depend_only_on_lower_layers` in `context-trace/src/positioned/tests/guard.rs`).
- The `context-*-legacy` crates hold the frozen row model (no new code). Their test suites are ignored by default; run them with `cargo test -p context-trace-legacy -p context-search-legacy -p context-insert-legacy -p context-read-legacy --features legacy-tests,test-api`.
