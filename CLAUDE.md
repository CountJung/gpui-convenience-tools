# CLAUDE.md

이 파일은 Claude Code용 저장소 어댑터다. 공통 규칙과 프로젝트 설명은 `docs/` 문서를
정본으로 사용한다.

## 필수 참조

- `docs/README.md` — 사용자 문서와 문서 인덱스
- `docs/DEVELOPMENT_GUIDE.md` — Rust·GPUI·Windows·검증 규칙
- `docs/PROJECT_MAP.md` — 모듈 책임과 줄 수 추적
- `docs/MASTER_PLAN.md` — 계획·완료 이력
- `docs/TODO.md` — 고유 작업 ID 기반 대기열

## Claude Code 전용 잊기 쉬운 항목

- Windows Claude Code의 실제 화면 표면은 `CLAUDE_LOCAL`이다. Computer Use의 `sky.*`나
  native pipe를 시도하지 말고 저장소 하네스를 사용한다.
- 시각 검증 전에는 앱을 중지하고, 빌드 후 격리된 데이터 루트로 새 세션을 시작한다.
- 시각 검증 종료 후 기록된 프로세스와 작업 전용 임시 데이터 루트가 정리됐는지 확인한다.
- 공통 규칙을 이 파일에 복사하지 않는다. 변경은 `docs/DEVELOPMENT_GUIDE.md`에서 한다.
