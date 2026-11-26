# Examples: user applications and tool usage

Part of [20261126_PLAN_CONTEXT_TOOLS](README.md). Output shown is
illustrative (format per [02_CLI.md](02_CLI.md) §2.3); indices and counts
are made up. These examples double as acceptance scenarios and as material
for the stage-6 READMEs.

## 1. Standalone CLI

### 1.1 Index a folder of notes and search it (text indexing, text search)

```bash
# one input per file, in directory order
context-cli create notes -f ~/notes --recursive --glob '*.md'
context-cli info notes
# notes  (.context-engine/notes.ctx)  412 inputs, 1 093 812 chars, 96 211 vertices

# where does a phrase occur? (concordance)
context-cli search notes "context engine" --view occurrences --width 30

# add today's note later; saved atomically, nothing written on error
context-cli read notes -f ~/notes/2026-11-26.md

# script use: exit 1 if missing
context-cli search notes "TODO(linus)" --fail-if-missing -q && echo "open todos"
```

### 1.2 Explore the contexts of a word (context exploration)

```bash
context-cli create poems -f poems.txt --split paragraphs
context-cli search poems "the sea" --view contexts --limit 10
context-cli search poems "the sea" --view parents         # direct parents only
context-cli vertex poems "the sea"                         # composition and parents
context-cli vertex poems '#4711'                           # by index
```

### 1.3 Small graphs: draw and export

```bash
context-cli create demo "abcabca" "bcab"
context-cli show demo                                   # ASCII layers
context-cli show demo --format mermaid -o demo.md       # fenced mermaid block for docs
context-cli show demo --format dot | dot -Tsvg > demo.svg
context-cli show notes --root "context engine" --depth 2 --format dot -o ce.dot  # subgraph of a big graph
context-cli export demo --format v3 -o demo.v3.json     # oracle interchange format
```

### 1.4 Pipelines and stdin

```bash
git log --format=%s | context-cli create commits --split lines
journalctl -u myservice --since today -o cat | context-cli read logs --create --split lines
context-cli search commits "fix(" --view occurrences --format json \
  | jq -r '.ok.search.occurrences[].snippet'
```

### 1.5 Compare the reader with the oracle (developer use)

```bash
context-cli create r "abcabca" "bcab"
context-cli export r --format v3 -o r.json
cargo run -p ngrams --example oracle -- "abcabca" "bcab" > o.json   # hypothetical helper
diff <(jq -S . r.json) <(jq -S . o.json)
```

## 2. Server and client

### 2.1 Keep a large graph in memory for a shell session

```bash
context-cli serve wiki --preload &                 # prints {"listening":"http://127.0.0.1:7878",...}
context-cli shell wiki                             # auto-discovers the server
context> search "general relativity" --view contexts --limit 5
context> read -f new_article.txt
context> :events on
```

Meanwhile, in another terminal, one-shot commands use the same server
without reloading:

```bash
context-cli search wiki "black hole" --view occurrences   # note: using server …
```

### 2.2 HTTP from any language

```bash
curl -s localhost:7878/v1/execute -d '{"id":1,"command":"search","graph":"wiki","query":"quantum","view":"summary"}'
curl -s 'localhost:7878/v1/graphs/wiki/search?q=quantum&view=occurrences&limit=3'
curl -s --data-binary @paper.txt 'localhost:7878/v1/graphs/wiki/read?label=paper.txt'
curl -s 'localhost:7878/v1/graphs/wiki/render?root=quantum&depth=1&format=dot' | dot -Tpng > q.png
```

```python
import requests
r = requests.post("http://127.0.0.1:7878/v1/execute",
                  json={"command": "search", "graph": "wiki", "query": "entropy", "view": "contexts"})
for c in r.json()["ok"]["search"]["contexts"]:
    print(c["width"], c["snippet"])
```

### 2.3 WebSocket (viewer frontend, live dashboards)

```js
const ws = new WebSocket("ws://127.0.0.1:7878/v1/ws");
ws.onopen = () => {
  ws.send(JSON.stringify({ id: 1, subscribe: { graph: "wiki", topics: ["graph_changed", "read_progress"] } }));
  ws.send(JSON.stringify({ id: 2, command: "render", graph: "wiki", format: "json", root: { text: "entropy" }, depth: 2 }));
};
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.event === "graph_changed") refresh(msg.generation);
  else if (msg.id === 2) draw(msg.ok.render);
};
```

Types come from `packages/context-types` (ts-rs).

### 2.4 Shared team index on a host

```bash
context-cli serve docs --http 0.0.0.0:7878 --token "$CONTEXT_TOKEN" --save interval=60s
CONTEXT_SERVER=http://indexhost:7878 CONTEXT_TOKEN=... context-cli search docs "deploy key"
```

## 3. MCP (agents)

### 3.1 Agent explores a code base's vocabulary

Configured as in [05_MCP.md](05_MCP.md) §5.3. Typical tool calls:

```json
{"tool": "context_create_graph", "arguments": {"graph": "repo", "texts": []}}
{"tool": "context_read", "arguments": {"graph": "repo", "files": ["src/lib.rs", "src/main.rs"]}}
{"tool": "context_search", "arguments": {"graph": "repo", "query": "fn parse_", "view": "occurrences", "limit": 10}}
{"tool": "context_search", "arguments": {"graph": "repo", "query": "Result<", "view": "contexts", "limit": 5}}
{"tool": "context_render", "arguments": {"graph": "repo", "root": "impl Display for", "depth": 1}}
```

### 3.2 Agent and human share one graph

```bash
context-cli serve research --preload &
# agent config: "args": ["mcp", "--server", "http://127.0.0.1:7878"]
context-cli shell research     # human sees the agent's reads as graph_changed events
```

## 4. Out of scope (future features, for orientation)

These are **not** part of this plan; listed to keep the command and
protocol design open for them.

| Feature | Example | Needs |
|---|---|---|
| Context reconstruction / completion | `context-cli complete notes "the quick br"` → `own fox jumps (0.62)`, `own dog (0.21)` | frequency/probability model over parent occurrences; "most likely contexts" ranking |
| Gap filling | `context-cli infer notes "the ___ fox"` → `quick (0.7)`, `brown (0.2)` | search with wildcards; context ranking |
| Fuzzy / approximate search | `search notes "contxt engine" --fuzzy 1` | edit-distance search over the graph |
| Wildcard / regex queries | `search notes "fn .*_test"` | automaton-driven traversal |
| Streaming ingestion | `tail -F app.log \| context-cli read logs --stream` with early commit | streaming retention (roadmap step 5), stability marks |
| Deleting inputs | `context-cli forget notes --input 17` | retention recomputation; not supported by the read-only-growth model |
| Merging graphs | `context-cli merge a b -o ab` | re-read or a positioned graph union |
| Compression statistics | `context-cli stats notes --compression` | vertex reuse metrics, MDL-style cost |
| 2D inputs | `context-cli create img --image tiles.png` | 2D `Space` abstraction in positioned paths |
| Full viewer telemetry | `subscribe {topics:["telemetry"]}` streaming search/read traversal steps into the viewer | positioned search/read visualization events |
| Semantic search | `search notes "ocean" --related` → `sea`, `waves` | context-similarity measures between vertices |
