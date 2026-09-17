---
name: gpui-rust-ui
description: GPUI UI 구현 절차를 제공하고 공통 규칙은 저장소 문서 정본으로 연결한다.
version: 3.0
---

# GPUI UI implementation adapter

이 스킬은 **절차와 점검 순서만** 담는다. 테마 토큰, 레이아웃, 패널 구조, 1,000줄 리팩터링,
테스트·시각 검증의 판정 기준은 `docs/DEVELOPMENT_GUIDE.md`가 단일 정본이다.

## 구현 순서

1. `docs/README.md`와 `docs/DEVELOPMENT_GUIDE.md`에서 프로젝트 목적·GPUI 규칙을 읽는다.
2. `docs/PROJECT_MAP.md`의 공용 UI 유틸 인벤토리를 확인하고 기존 프리미티브를 재사용한다.
3. 기능의 흐름을 판단한다.
   - 목록과 설정을 독립적으로 탐색하면 `h_resizable` + 각 영역 `window::scroll_pane`
   - 선택 → 설정 → 실행 → 결과가 이어지면 전체 너비 단일 `scroll_pane`
4. 새 패널은 `ActivePanel`, `NAV_TOOLS`, `window/<feature>.rs`, `fills_height`를 함께 갱신한다.
5. 렌더 함수에는 비즈니스 로직과 부작용을 넣지 않고, 이벤트 핸들러에서 상태를 변경한다.
6. 코드 변경 후 GPUI 자체 테스트를 먼저 실행하고, 실제 화면 변경이면 저장소의
   `gpui-visual-check` 절차로 시각 검증한다.
7. 완료한 작업은 `docs/TODO.md`에서 제거하고 `docs/MASTER_PLAN.md`에 기록한다.

## GPUI UI 작업에서 자주 잊는 항목

- 색상 하드코딩·`card`·`destructive` 토큰을 사용하지 않는다.
- 긴 스크롤 컨텐츠에는 `size_full`/세로 `flex_1`을 남용하지 않고 자연 높이와 `w_full`을 유지한다.
- 클릭 가능한 요소에는 안정적인 ID/debug selector를 부여해 GPUI 테스트에서 조작할 수 있게 한다.
- UI 변경은 화면 캡처만으로 완료 처리하지 않는다. 관련 `#[gpui::test]` 수용 기준과 실제
  검증 표면의 한계를 함께 기록한다.
