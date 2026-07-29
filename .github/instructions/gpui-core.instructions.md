---
description: "Use when working on gpui-convenience-tools Rust code, GPUI UI, gpui-component theme tokens, workspace setup, phase implementation, or Windows platform integration."
name: "GPUI Convenience Tools Core Rules"
applyTo: "app/src/**/*.rs,**/*.toml,MasterPlan.md,TODO.md,PROJECTMAP.md"
---
# gpui-convenience-tools Core Rules Bridge

이 문서는 메인 지침으로 연결하기 위한 브리지 문서다.
실제 규칙 정의와 변경은 다음 파일만 단일 정본으로 유지한다.

- `.github/copilot-instructions.md`

아래 항목은 메인 지침의 핵심 범위를 요약한다.

- 정체성: GPUI 기반 다용도 데스크탑 보조 도구 모음(편의 기능을 담는 그릇)
- 편의 기능은 독립 패널 + 스플리터(기능 영역 / 설정 영역) 구조로 추가
- **소스 파일 1,000줄 초과는 구조 리팩터링 트리거**(단순 분할 아님) — 중복 제거 →
  오배치 책임 이동 → 그래도 크면 책임 단위 분할 순서로 처리하고 `PROJECTMAP.md` 갱신
- **중복 헬퍼는 상시 공용 유틸로 승격** — 이름이 달라도 같은 일을 하면 중복,
  3개 파일 이상이면 즉시 승격(UI 프리미티브는 `window/ui.rs`)
- **「즉시」 판정 항목은 그 작업 안에서 처리**하고 대기열로 미루지 않는다
- 완료 이력은 `MasterPlan.md`, 미착수 대기열은 `TODO.md` (별도 진행 문서 없음)
- GPUI 0.2.2, gpui-component 0.5.1 호환 사용
- 테마 토큰 기반 UI 스타일링(`card` / `destructive` 미사용)
- 스위치는 `window::ui::toggle_switch`, 테마 변경은 `crate::theme::change_theme` 사용
- `scroll_pane` 컨텐츠 루트는 자연 높이 유지(`w_full`; `size_full`/세로 `flex_1` 금지)
- 설정 저장은 `config::update_config` 단일 경로 사용
- GPUI UI 변경 시 정본의 「GPUI 자체 테스트 컨텍스트 필수 검증」과
  「GPUI 시각 검증 및 독립 크로스체크」 절 적용
- 작업 후 Error Reviewer 기반 오류 전용 리뷰
