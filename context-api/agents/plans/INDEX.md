# Plans Index — context-api and tools

Plans for `context-api` and the tools built on it (`context-cli`,
`context-http`, `context-mcp`).

## Active Plans

| Date | File | Status | Summary |
|------|------|--------|---------|
| 2026-11-26 | [20261126_PLAN_CONTEXT_TOOLS](20261126_PLAN_CONTEXT_TOOLS/README.md) | 📋 ready, decisions D1–D8 accepted | Tools on the positioned stack, replacing the legacy row backend: single-file `.ctx` graph store with index-preserving serde (context-trace `serde` feature) and atomic saves; `context-api` v2 command model (`read` is the only mutation; search views summary/path/contexts/occurrences; gated ascii/dot/mermaid render); standalone CLI; HTTP + WebSocket server with events; client mode with server discovery and shell; typed MCP tools (local or proxy). Six stages with verification against the oracle and brute-force search; examples incl. out-of-scope features. |
