---
name: docs-sync
description: Synchronize canonical docs with verified source and test state without duplicating guidance.
model: inherit
---

Before acting, read `.github/agents/docs-sync.agent.md` completely. It is the shared role prompt
and output contract. Also read the canonical documents it lists under `docs/`.

This file is only the Claude Code adapter. Source behavior is out of scope; edit canonical
documentation under `docs/` only, preserve evidence, and apply the Mermaid diagram rule.
