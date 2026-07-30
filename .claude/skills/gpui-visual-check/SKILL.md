---
name: gpui-visual-check
description: GPUI 레이아웃·시각 회귀를 조사하고 실제 화면으로 검증한다. UI를 바꿨거나, 사용자가 스크린샷으로 레이아웃 이상(폭이 안 맞음·잘림·스크롤 안 됨)을 보고했거나, 폭·높이·스크롤 회귀를 재현해야 할 때 사용한다.
---

절차 정본은 **`.github/skills/gpui-visual-check/SKILL.md`**다. 먼저 그 파일을 끝까지 읽고
그대로 수행한다. 이 파일은 Claude Code 어댑터이며 아래 표면 사항만 덧붙인다.

판정 기준과 증거 요건은 `.github/copilot-instructions.md`의 「실행 표면 하드 게이트」와
「GPUI 시각 검증 및 독립 크로스체크」가 정본이다.

## 표면

Windows의 Claude Code는 `CLAUDE_LOCAL`이다. ChatGPT 데스크톱의 Computer Use(`sky.*`,
native pipe)는 **존재하지 않으므로 초기화·재시도하지 않는다.** 대신 저장소가 소유한
`scripts/Invoke-ClaudeVisualCheck.ps1`으로 직접 검증하고 `PASS`/`FAIL`/`BLOCKED`를 판정한다.
`DESKTOP_PENDING`으로 인계하지 않는다.

비Windows이거나 VS Code의 Codex·Copilot 확장이면 `IDE` 표면이므로 화면 조작을 시도하지 않고
`IDE_VERIFIED / DESKTOP_PENDING`으로 인계한다.

## 도구 대응

| 절차 | 사용할 도구 |
| --- | --- |
| 하네스 실행 (`Start`/`Click`/`Wheel`/`Capture`/`Resize`/`Stop`) | PowerShell |
| 캡처 PNG 관찰 | Read (PNG 경로를 그대로 넘기면 이미지로 보인다) |
| 진단 테스트 작성·삭제 | Edit / Write, 되돌릴 때는 `git checkout <file>` |
| 시드 `config.json` 작성 | **Write** (Bash 힙독은 백슬래시가 먹혀 JSON이 깨진다) |

## 사용자 화면 점유

`SendInput`은 데스크톱 전역 입력이라 실제 커서가 움직이고 대상 창이 포그라운드로 올라온다.
**사용자가 다른 작업을 하는 중에는 실행하지 않는다.** 입력이 필요한 검증을 시작하기 전에
사용자에게 알린다. 캡처만 하는 경우(`Capture`)는 `PrintWindow`가 창 단위로 가져오므로
포그라운드가 필요 없고 사용자를 방해하지 않는다.

## 독립 크로스체크

구현 담당자 1차 검증 뒤 2차 독립 검증이 필요하면
`.claude/agents/ui-visual-reviewer.md`를 사용한다. 그 에이전트는 자체 세션을 열므로
**먼저 `-Action Stop`으로 내 세션을 정리한 뒤** 넘긴다(활성 세션이 있으면 `Start`가 거부된다).
