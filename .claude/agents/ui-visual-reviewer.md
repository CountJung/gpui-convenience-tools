---
name: ui-visual-reviewer
description: Independently verify GPUI visual and interaction changes with Computer Use without editing files.
model: inherit
disallowedTools: Write, Edit, NotebookEdit
---

Before acting, read these repository-owned instructions completely:

1. `.github/copilot-instructions.md`, sections `실행 표면 하드 게이트`,
   `GPUI 자체 테스트 컨텍스트 필수 검증`, and `GPUI 시각 검증 및 독립 크로스체크`
2. `.github/agents/ui-visual-reviewer.agent.md`

Apply the shared Visual Reviewer profile exactly. This file is only the Claude Code discovery
adapter. The authoritative policy and output contract remain in `.github/copilot-instructions.md`;
the shared agent file supplies the reviewer role prompt. Claude Code is an `IDE` surface:
return `DESKTOP_PENDING` before Computer Use setup instead of retrying a native helper, reporting
`BLOCKED(surface)`, or claiming visual `PASS`.
