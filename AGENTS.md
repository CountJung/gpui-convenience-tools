# AGENTS.md

이 파일은 에이전트별 진입점이다. 공통 프로젝트 규칙과 현재 상태를 복사하지 않고
`docs/`의 정본을 참조한다.

## 착수 경로

1. [문서 등록부](docs/README.md)에서 책임과 관련 열린 [작업](docs/TODO.md)을 확인한다.
2. [공통 규칙](docs/DEVELOPMENT_GUIDE.md)의 해당 변경 기준을 적용한다.
3. 구조는 [PROJECT_MAP](docs/PROJECT_MAP.md), 범위는 [MASTER_PLAN](docs/MASTER_PLAN.md),
   검증은 [VERIFICATION](docs/VERIFICATION.md)의 **관련 절만** 읽는다. 전체 필독 목록이 아니다.

## AGENTS 전용 잊기 쉬운 항목

- 사용자 승인 없이 파일 삭제·대량 덮어쓰기·릴리즈·외부 발송을 실행하지 않는다.
- 사용자가 보고한 문제를 재현하지 못했으면 재현한 것처럼 보고하지 않는다.
- 검증용 앱과 파일 작업은 `GPUI_CONVENIENCE_TOOLS_DATA_DIR`로 격리한다. 사용자
  `%APPDATA%`를 검증 대상으로 사용하지 않는다.
- 문서 정본의 내용을 이 파일에 다시 작성하지 않는다. 프로젝트 규칙을 바꿀 때는
  `docs/DEVELOPMENT_GUIDE.md`를 먼저 갱신한다.
