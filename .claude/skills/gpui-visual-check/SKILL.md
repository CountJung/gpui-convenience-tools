---
name: gpui-visual-check
description: GPUI 레이아웃·시각 회귀를 조사하고 실제 화면으로 검증한다. UI 변경이나 폭·잘림·스크롤 문제를 확인할 때 사용한다.
---

절차 정본은 `.github/skills/gpui-visual-check/SKILL.md`다. 먼저 끝까지 읽는다.
수용 기준은 `docs/DEVELOPMENT_GUIDE.md`의 「실행 표면 하드 게이트」와
「GPUI 시각 검증 및 독립 크로스체크」를 따른다.

## 도구 대응

에이전트 이름 대신 현재 런타임의 실제 지원 도구로 검증 경로를 선택한다.
Windows 로컬 하네스를 사용할 수 있으면 `scripts/Invoke-ClaudeVisualCheck.ps1`을
PowerShell로 실행하고 캡처 PNG를 Read로 관찰한다. 없는 native pipe를 만들지 않는다.
자동 테스트만 가능한 환경에서는 꼭 필요한 화면·OS 조건만 후속으로 남긴다.

진단 테스트를 제거할 때는 자신이 추가한 부분만 Edit/패치로 제거한다.
파일 전체를 되돌려 기존 변경을 잃지 않는다. 시드 JSON은 Write로 작성한다.

## 선택 검토

Visual Reviewer가 필요한 작업이면 `.claude/agents/ui-visual-reviewer.md`를 사용한다.
실제 재조작이 필요할 때만 자신의 세션을 먼저 Stop하고 검토자의 새 격리 실행을 요청한다.
기존 증거 검토와 새 독립 재현을 구분한다. 내부 이벤트·결과 테스트가 충분한 작업에
별도 데스크톱 반복 확인을 추가 완료 조건으로 만들지 않는다.
