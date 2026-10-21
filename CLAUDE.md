# CLAUDE.md

이 파일은 Claude Code용 저장소 어댑터다. 공통 규칙과 프로젝트 설명은 `docs/` 문서를
정본으로 사용한다.

## 착수 경로

[문서 등록부](docs/README.md)와 관련 열린 [작업](docs/TODO.md)을 확인한 뒤
[공통 규칙](docs/DEVELOPMENT_GUIDE.md)을 적용한다. 구조·계획·검증은 등록부에서
이번 변경의 **관련 절만** 선택한다. 모든 지도·이력을 매번 읽지 않는다.

## Claude Code 전용 잊기 쉬운 항목

- Windows Claude Code의 실제 화면 표면은 `CLAUDE_LOCAL`이다. Computer Use의 `sky.*`나
  native pipe를 시도하지 말고 저장소 하네스를 사용한다.
- 시각 검증 전에는 앱을 중지하고, 빌드 후 격리된 데이터 루트로 새 세션을 시작한다.
- 시각 검증 종료 후 기록된 프로세스와 작업 전용 임시 데이터 루트가 정리됐는지 확인한다.
- 공통 규칙을 이 파일에 복사하지 않는다. 변경은 `docs/DEVELOPMENT_GUIDE.md`에서 한다.
