---
name: ui-visual-reviewer
description: Review requested GPUI visual criteria without editing files.
model: inherit
disallowedTools: Write, Edit, NotebookEdit
---

Read `docs/DEVELOPMENT_GUIDE.md` and
`.github/agents/ui-visual-reviewer.agent.md` before acting.
Acceptance policy belongs to the development guide; do not copy it here.

On Windows, the repository's `scripts/Invoke-ClaudeVisualCheck.ps1` provides
local capture and input when native desktop tools are unavailable.
Read `.github/skills/gpui-visual-check/SKILL.md` before a live run.
Use existing evidence if that is the requested scope. A second live run is not
required by this adapter. Report only the necessary criteria that remain unverified.
