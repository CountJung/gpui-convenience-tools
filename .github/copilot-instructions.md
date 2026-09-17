---
description: "Repository adapter for the canonical project documents."
name: "GPUI Convenience Tools Copilot Adapter"
applyTo: "app/src/**/*.rs,**/*.toml,docs/**/*.md"
---

# GitHub Copilot adapter

공통 프로젝트 규칙과 문서 정본은 `docs/` 아래에만 둔다. 작업을 시작할 때 다음 문서를
필요한 범위로 읽는다.

- `docs/README.md` — 사용자 관점과 문서 인덱스
- `docs/DEVELOPMENT_GUIDE.md` — 공통 코딩·아키텍처·검증 규칙
- `docs/MASTER_PLAN.md` — 단계 계획과 완료 이력
- `docs/PROJECT_MAP.md` — 실제 구조·줄 수·공용 유틸 지도
- `docs/TODO.md` — 미완료 작업과 고유 작업 ID

## Copilot 전용 잊기 쉬운 항목

- `applyTo`에 해당하는 소스 변경 전후에는 canonical 문서의 검증 명령을 실행한다.
- 문서 규칙을 이 파일에 복사하지 않는다. 공통 규칙을 바꿀 때는 먼저
  `docs/DEVELOPMENT_GUIDE.md`를 수정하고 이 파일은 참조만 유지한다.
- GPUI UI를 바꾸면 canonical 문서의 GPUI 자체 테스트·시각 검증 게이트를 적용한다.
- 완료한 작업은 `docs/TODO.md`에서 제거하고 `docs/MASTER_PLAN.md`에 이력을 남긴다.
