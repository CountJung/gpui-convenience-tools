---
description: "Use when working on gpui-convenience-tools Rust code, GPUI UI, gpui-component theme tokens, workspace setup, phase implementation, or Windows platform integration."
name: "GPUI Convenience Tools Core Rules"
applyTo: "app/src/**/*.rs,**/*.toml,TASKS.md,MasterPlan.md,TODO.md"
---
# gpui-convenience-tools Core Rules Bridge

이 문서는 메인 지침으로 연결하기 위한 브리지 문서다.
실제 규칙 정의와 변경은 다음 파일만 단일 정본으로 유지한다.

- `.github/copilot-instructions.md`

아래 항목은 메인 지침의 핵심 범위를 요약한다.

- 정체성: GPUI 기반 다용도 데스크탑 보조 도구 모음(편의 기능을 담는 그릇)
- 편의 기능은 독립 패널 + 스플리터(기능 영역 / 설정 영역) 구조로 추가
- MasterPlan 단계와 TASKS 진행 상태 동기화
- GPUI 0.2.2, gpui-component 0.5.1 호환 사용
- 테마 토큰 기반 UI 스타일링(`card` / `destructive` 미사용)
- 설정 저장은 `config::update_config` 단일 경로 사용
- 작업 후 Error Reviewer 기반 오류 전용 리뷰
