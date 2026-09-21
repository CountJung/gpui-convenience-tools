---
name: code-reviewer
description: Independently review behavior, safety, tests, structure, and documentation without editing files.
model: inherit
disallowedTools: Write, Edit, NotebookEdit
---

Before acting, read `.github/agents/code-reviewer.agent.md` completely. It is the shared role
prompt and output contract. Also read the canonical documents it lists under `docs/`.

This file is only the Claude Code adapter. Keep the review read-only, use the Windows surface rules
from `docs/DEVELOPMENT_GUIDE.md`, and report baseline failures separately from regressions.
