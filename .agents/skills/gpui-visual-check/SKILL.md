---
name: gpui-visual-check
description: GPUI 화면·레이아웃 결함을 조사하고 필요한 자동 테스트와 실제 화면 증거를 확인한다.
---

절차는 `.github/skills/gpui-visual-check/SKILL.md`를 먼저 읽는다.
검증 수준과 완료 판정의 정본은 `docs/DEVELOPMENT_GUIDE.md`의
「실행 표면 하드 게이트」·「GPUI 시각 검증 및 독립 크로스체크」다.

현재 런타임에서 지원하는 도구를 선택한다. Windows 로컬 하네스 경로라면
`scripts/Invoke-ClaudeVisualCheck.ps1`로 실행·캡처하고 저장된 PNG를 관찰한다.
에이전트 이름만으로 네이티브 Computer Use의 존재 여부를 가정하지 않는다.

전역 입력이 필요한 경우 짧은 화면 점유를 알리고 사용자 작업 중에는 보내지 않는다.
커서·포커스 복원 결과는 공통 지침대로 기록한다. 표시만 확인하면 캡처로 충분하다.

Visual Reviewer가 필요한 작업이면 `.github/agents/ui-visual-reviewer.agent.md`를 따른다.
새 실제 검증을 맡길 때만 내 하네스 세션을 먼저 Stop하고 자체 세션을 넘긴다.
기존 증거 검토에 별도 앱 실행을 요구하지 않는다.
