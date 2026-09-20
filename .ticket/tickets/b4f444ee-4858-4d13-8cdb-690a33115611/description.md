Move the context-stack submodule from crates/context-stack to context-stack at the repository root, remove the deprecated humans/, agents/, scripts/, and tools/http/ directories, and update the workspace configuration to the new layout.

Acceptance criteria:
- context-stack lives at the repository root.
- crates/ is removed if empty after the move.
- humans/, agents/, scripts/, and tools/http/ are removed.
- workspace/manifests/configuration no longer reference the old paths.
- a focused workspace validation command succeeds after the move.
