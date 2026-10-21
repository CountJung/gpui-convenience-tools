# gpui-convenience-tools — 단계 계획

GPUI 기반 다용도 데스크탑 보조 도구 모음의 단계 범위와 현재 착수 경로를 안내한다.
구현 상세·회차별 결과는 [구현 이력](IMPLEMENTATION_HISTORY.md)에 보존하며,
현재 미완료 항목의 상세 정본은 [TODO.md](TODO.md)다.

## 현재 단계

- 광고 창 축소·카카오톡 자리 회수: 사용자 실제 확인으로 수용 종료.
- 사용자 설정 복원: 격리 검증 완료, 기존 사용자 프로필 재시작 확인이 남아 있다.
- VirtualBox 오프라인 탐색: NTFS 탐색·복사와 화면 수용 완료.
  네이티브 VDI 파일 선택·취소 확인이 남아 있다.
- 파일 동기화: 제외 패턴·진행률·실시간 감시·링크/정션·이력까지 구현·검증 이력으로 이동.
- OpenSSL 암복호화 `SSL-001`: 2026-10-21 사용자 확인으로 수용 종료.
  구현과 반영 상태는 [구현 이력](IMPLEMENTATION_HISTORY.md#ssl001-implementation-stage),
  검증·사용자 확인은 [검증 기록](VERIFICATION_RECORDS.md#ssl001-evidence)을 따른다.
- 구조 개선: 스플리터 입력→저장→재로드 단언 보강이 남아 있다.
- 신규 편의 기능·macOS 네이티브 확장: 범위 결정과 환경 확보 후 진행한다.

위 상태는 새 제품 검증 결과가 아니다. 정확한 열린 ID와 다음 조작은
[TODO.md](TODO.md), 게이트 상태는 [VERIFICATION.md](VERIFICATION.md)를 따른다.

## 경계와 정본

- 현재 코드 구조·공용 유틸: [PROJECT_MAP.md](PROJECT_MAP.md)
- 패널 구성·상태 전달·설정 저장·구조 개선 규칙: [DEVELOPMENT_GUIDE.md](DEVELOPMENT_GUIDE.md)
- VDI 지원·안전 결정: [VIRTUAL_DISK_BACKENDS.md](VIRTUAL_DISK_BACKENDS.md)
- 빌드·E2E 실행법: [VERIFICATION.md](VERIFICATION.md)
- 지난 검증의 명령·수치·해시·미검증 경계: [VERIFICATION_RECORDS.md](VERIFICATION_RECORDS.md)
- 사용·설치·릴리즈 명령: [README.md](README.md)

## 구현 완료 단계

[구현 이력](IMPLEMENTATION_HISTORY.md)의 각 단계에서 당시 구현·커밋·검증 경계를 찾는다.
이력의 '진행 예정'·'착수' 표기는 당시 상태이며 현재 대기열이 아니다.

## 2026-10-19 — 문서 체계 간소화

전역 docs-canon 기준을 적용해 현재 단계·열린 작업·현재 검증 절차와 과거 근거를 분리했다.
이 작업은 문서 전용이며 제품 실행·설정·VDI 안전 계약을 변경하지 않았다.
검증은 문서 링크/도달성·열린 ID 대응·구조 assertion·diff 형식과 내용 검토로 한정한다.
