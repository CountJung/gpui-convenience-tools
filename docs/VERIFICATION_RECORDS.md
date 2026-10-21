# 검증 결과 기록

이 문서는 당시 코드·환경에서 수행한 검증 근거의 정본이다. 과거 통과 수·화면 관찰을
현재 코드의 재검증 결과로 보지 않는다. 현재 절차와 열린 게이트는 [VERIFICATION.md](VERIFICATION.md),
남은 일은 [TODO.md](TODO.md)를 따른다. 2026-10-19 문서 정리에서 원문 근거를 이관했다.

<a id="e001-evidence"></a>

## 2026-10-21 — E-001 클립보드 히스토리 통합 검증

- 대상·환경: `d80b1e1` 이후 Windows E-001 통합 작업 트리. 기본 수집 OFF,
  합성 텍스트/PNG·GPUI 메모리 클립보드·격리 저장소만 검증에 사용했다.
- 자동: `cargo test --all-targets --all-features --offline` **269 passed / 8 ignored /
  0 failed**. E-001 필터 30개, all-targets/all-features check와 clippy `-D warnings`,
  변경 Rust 파일 fmt 통과. OS 기본 본문 읽기는 테스트에서 실행하지 않는다.
- GPUI는 수집 opt-in·유형/민감정보/제외 정책·검색·선택·텍스트/PNG 재복사·고정·삭제·
  확인형 초기화·한도/만료·손상 차단·설정 자동 저장과 복원·worker 단일 실행을 단언했다.
  설정 적용 버튼의 스크롤 끝 도달과 920/1000/1500px 폭, 기본 창의 첫 이력 가시성을 확인했다.
- 독립 리뷰 지적은 모두 수정했다. 만료 재복사의 디스크 정리, 재복사→초기화 후 재수집 기준,
  OS 데이터 크기 검사 이전 복사, worker 대기 중 만료되는 중복 항목의 저장 누락을 각각
  회귀로 남겼다. 마지막 회귀는 일반 중복/동일 fingerprint 두 경로에서 PNG 삭제뿐 아니라
  원문 index의 정리까지 단언한다. Sagan 최종 Code Reviewer 정적 PASS.
- Windows 본문 경계는 형식·GlobalSize를 먼저 검사하고 잠금 범위에서 제한된 본문만
  읽는다. 읽기 전후 sequence·privacy marker 변경과 이미지 OFF의 무복사를 가짜 source로
  단언했다. 실제 개인정보 수집·시스템 클립보드 쓰기 성공의 증거는 아니다.
- 구조: 설정 테스트 20개를 `config/tests.rs`, 시작·설정 복원을 `app/startup.rs`로 옮겼다.
  Noether가 `d80b1e1`과 대조해 기존 복원·루프 시작·Drop 보존을 확인했고 Error Reviewer
  정적 검토에서도 추가 오류 후보가 없었다. 실행 진단의 근거는 위 check/clippy다.
- 최종 별도 출력 release: `cargo rustc -p gpui-convenience-tools --release --offline --
  -o .../target/clipboard-validation.exe` 성공. 출력 경로 관련 Cargo 경고 2개만 있었다.
  SHA-256 `3fa071f80eb48d244ae0ce698357992bbece2361af370a0bee316a10db184b9b`.
- 하네스 `-SeedConfig .../e001-config.json -SeedClipboardHistory .../e001-history.json
  -InitialPanel ClipboardHistory`로 수집 OFF·합성 2건을 격리 실행했다.
  `target/visual-validation/captures/e001-default-131722.png`(1000×700)와
  `e001-minimum-131729.png`(920×480)를 직접 열어 이력 우선 배치·보이는 텍스트와 버튼의
  비겹침을 확인했다. 최소 높이의 아래 내용은 스크롤이 필요하며 끝 도달 근거는 자동 테스트다.
  Noether도 두 PNG의 보이는 배치를 기존 증거로 검토해 PASS로 판정했다.
- 검증 PID 32516 종료, 세션 루트 `gpui-claude-visual-aaf6677faeb8492c884376e181d496ea`
  삭제(`sessionRootExists=false`), 사용자 PID 10208 유지. 전역 키/마우스 입력 없이 소유 창
  캡처/리사이즈만 수행했고 사용자 포커스 변경(`skipped-focus-changed`)은 덮지 않았다.
- 측정은 E-001 추적/신규 소스 103개·32,174줄·최대 907줄·800줄 이상 4개로 대조했다.
  미통합 E-003/E-004 미추적 파일은 제외했으므로 전체 작업 트리 구조 assertion의 통과로
  표현하지 않는다. 전체 대조는 남은 E 통합 후 수행한다.
- 실제 사용자 클립보드 읽기/쓰기·macOS 실행·장시간 I/O 지연 측정은 미실행이다.
  개인정보 검증 범위를 확장하지 않고 내부/경계 계약과 격리 실제 화면의 증거를 구분한다.
- 부모의 최종 전체 회귀 재실행도 269 passed / 8 ignored / 0 failed, strict clippy 통과.
  문서 형식 검사 38개 Markdown 오류 0, 열린 ID 대응 10/10, diff 검사 통과.

<a id="e002-evidence"></a>

## 2026-10-21 — E-002 프로세스 모니터 통합 검증

- 대상·환경: `4c368f5` 이후 Windows 작업 트리의 E-002 통합. 사용자 앱 PID 10208은
  유지하고 검증 앱만 `GPUI_CONVENIENCE_TOOLS_DATA_DIR`로 격리했다.
- 자동: 기능 테스트 9개·설정 테스트 2개 통과. 전체 회귀 **239 passed / 8 ignored /
  0 failed**; offline check와 clippy `-D warnings` 통과. 소유 테스트 실행 파일을 자식으로
  띄워 생성 시각·경로·PID 신원 변경 거부 후 실제 종료를 확인했다. 사용자 프로세스는
  종료하지 않았고 보호 대상은 정책/실패 테스트로 확인했다.
- 조회 실패 회귀: 초기·후속 갱신 0건을 실패로 처리하고 이전 행에 오래된 정보 경고를
  표시한다. 선택 취소와 다음 스캔의 CPU 워밍업 재시작을 단언했다. Noether의 해당 수정
  Code Reviewer 재검토 PASS. Error Reviewer 정적 검토는 추가 오류 후보 없음이며
  실행 진단 PASS의 근거는 위 check·clippy·테스트 결과다.
- 첫 실제 release 캡처에서 주기 도움말이 한 글자 폭으로 압축되는 결함을 발견했다.
  초기 해시 `44f14e37d949342cb24f34a7f1d412b31e315f5a47100ac661d62fd144a189aa`의
  `e002-default-124436.png`·`e002-minimum-124437.png`는 실패 재현 증거이지 통과 증거가 아니다.
  검증 PID 7888 종료·세션 루트 제거를 확인했고 전역 입력은 보내지 않았다.
  포커스는 사용자 변경으로 `skipped-focus-changed`여서 강제 복원하지 않았다.
- 수정 후 필터 GPUI 5개 통과. 도움말을 자연높이 독립 행으로 옮기고
  920/1000/1500px의 읽을 수 있는 폭·후속 상태행 비겹침을 단언했다.
- 최종 별도 출력 release 빌드 성공. SHA-256
  `13ccbffe60dfd15573c4959c44b05d2def90b56da2446746bda6be186662af47`.
  `target/visual-validation/captures/e002-fixed-default-124903.png`(1000×700)와
  `e002-fixed-minimum-124904.png`(920×480)를 직접 열어 도움말·표 열의 겹침 해소를 확인했다.
  최소 높이는 목록이 아래로 이어져 세로 스크롤이 필요하며 끝 도달은 GPUI 자동 테스트로 확인했다.
  최종 검증 PID 42636 종료·세션 루트 제거, 사용자 PID 10208 유지. 전역 입력 없이
  캡처/리사이즈만 수행했고 사용자 포커스 변경을 덮지 않았다.
- 수정 후 전체 회귀 재실행도 **239 passed / 8 ignored / 0 failed**. 구조 지도는
  추적 파일과 E-002 신규 파일만 89개/29,012줄로 대조했고 다른 E 미추적 파일은 제외했다.
- 실제 사용자 프로세스 종료·자동 승격·macOS 실행은 검증 범위 밖이다.
  최종 clippy `-D warnings`, 문서 형식 검사(38개 Markdown·오류 0), 열린 ID 대응
  **13/13**, diff 검사 통과. Noether도 최종 PNG의 보이는 도움말·표 열 배치를 확인했다.
  스크롤 끝 도달은 정지 PNG가 아닌 자동 테스트의 근거다.
- Git 후속: `d80b1e1` 커밋·원격 푸시 후 fetch, ahead/behind **0/0** 확인.
  이후 E-001 작업에서 구조 지도·기능 소스가 다시 바뀔 수 있으므로 위 해시별 검증과 구분한다.

## 2026-10-21 — 작업 목록 전역 규칙 재확인·충돌 해소

- 범위: SSL-001은 사용자 수용 종료 이력에 있고 열린 TODO·검증 매트릭스에는 없는 것을
  재확인했다. 별도 진행 중인 TIME·E 소스 변경은 이번 문서 작업의 검증 대상이 아니다.
- 전역 docs-canon 정본에 대기·차단 사유와 다음 행동 주체를 남기는 기준을 보강했다.
  Codex 어댑터와 Claude는 같은 정본을 참조한다. 다른 저장소의 기존 문서는 일괄 변경하지 않았다.
  전역 기록 예시에도 대기 이유·다음 주체를 추가해 사용자가 제공할 정보와 에이전트의 후속 작업을 구분했다.
- 프로젝트 지침의 「추정 진행률 금지」를 확인된 범위의 대략적 추정 허용으로 정정해
  「작업 목록 표기」와 충돌을 없앴다. 빌드 성공만으로 기능 완료를 주장하지 않는 제한은 유지했다.
- 형식 검사: `python -X utf8 .../check_docs.py . --changed`에서 38개 Markdown의
  상대 링크·표·도달성 오류 및 앵커·완료 TODO 경고 0. `Assert-ProjectDocs.ps1`은
  현재 병행 작업을 포함해 `active=15 matrix=15`. 전역 원본·Codex 어댑터의
  `quick_validate.py` 통과, `git diff --check` 오류 없음(기존 LF/CRLF 알림만 표시).
- 내용 검토: ID·진행률·남은 일·큰 작업의 하위 ID·차단 주체·완료 이관 기준과
  기존 수용 근거 보존을 확인했다. 이번 보강에서 제품 테스트·Git 반영은 수행하지 않았다.

<a id="ssl001-20261021-followup"></a>

## 2026-10-21 — SSL-001 Git 반영·관련 테스트 후속

- 메인 담당자 전달: SSL-001의 기존 변경을 `e065ae1`로 커밋·푸시 완료.
  Documentation Sync는 읽기 전용 `git show --no-patch --format=fuller e065ae1`,
  `git log -1`·`git rev-parse origin/main`으로 로컬 HEAD와 로컬 원격 추적 참조가
  `e065ae1144845568aaca4540db5abf8c8144ec4e`인 것을 확인했다. 새 fetch나 push는 하지 않았다.
- 메인 전달 재실행 결과: 오늘 `cargo test`의 `openssl` 필터 **20/20 통과**,
  확장 착수 전 docs assertion **12/12 통과**. 정확한 명령 옵션·실행 로그는 메인 후속 기록 대상이다.
  아래의 과거 전체 회귀 수치나 새 TIME·E 작업의 게이트 통과로 확대하지 않는다.
- 아래 수용 종료 당시의 C 미수행·승인 대기 문구는 당시 경위로 보존한다.
  현재 반영 상태는 이 후속 기록을 따른다. 사용자 설정·기존 미검증 경계는 변경하지 않는다.

<a id="time-e-20261021-progress"></a>

## 2026-10-21 — TIME-001·Phase E 착수 시점 기록

- 대상: `e065ae1` 이후 병행 구현 중인 Windows 작업 트리. 사용자가 Unix timestamp 및
  `E-001`~`E-004` 전체 구현·작업별 커밋·푸시를 승인했고 Git 작업은 메인 담당자가 수행한다.
- 메인 전달: TIME 코드 구현 후 관련 필터 테스트 **6 passed / 0 failed**.
  그중 기존 NTFS 테스트 1개가 포함되어 실제 추가 TIME 테스트는 5개다.
  소스에서 순수 변환 테스트 3개와 GPUI 변환·시간대 저장/재로드 테스트 2개를 확인했다.
  후속 명령·빌드·화면 결과는 [TIME 검증 기록](#time001-evidence)으로 연결한다.
- E 기능은 4개 담당자의 병행 구현·통합 전 단계다. 소스와 설정 기본값을 읽어
  [사용 안내 초안](README.md#phase-e-사용-안내-초안)을 작성했으며 테스트·OS 경계·화면 확인
  성공이나 완료 이력을 만들지 않았다. 최신 ID·게이트는 TODO·VERIFICATION 정본을 따른다.
- Documentation Sync 작업 범위: 허용된 안내·계획·공통 규칙·SSL 이력·이 기록만 갱신.
  TODO·VERIFICATION·PROJECT_MAP과 타 담당자의 소스 변경은 편집하지 않는다.
  제품 실행·테스트·사용자 설정 변경·checker·커밋·푸시는 수행하지 않는다.
  최종 문서 checker와 프로젝트 검사는 메인 담당자가 수행하고 결과를 별도로 전달한다.

<a id="time001-evidence"></a>

## 2026-10-21 — TIME-001 자동·Windows release 화면 확인

아래는 메인 담당자가 전달한 실행·화면 관찰 결과다. Documentation Sync는 소스·테스트를
읽어 문서화했으며 제품·테스트를 재실행하거나 캡처를 직접 열어 새 독립 검증을 하지 않았다.

- 대상·환경: `e065ae1` 이후 미커밋 TIME 구현, Windows 별도 출력 release 바이너리.
  E 기능은 병행 구현 중으로 이 검증의 완료 범위에 포함하지 않는다.
- 자동: `cargo test timestamp --offline -- --nocapture` **6 passed / 0 failed**.
  신규 TIME 순수 테스트 3개·GPUI 테스트 2개와 기존 NTFS 테스트 1개다.
  변환 왕복·소수/음수·잘못된 날짜, UI 양방향 변환·오류, 격리 시간대 저장/재로드·
  입력 폭을 확인했다. 신규 테스트 6개나 전체 회귀 통과로 계산하지 않는다.
- 빌드: `cargo rustc --release --offline -- -o target/timestamp-validation.exe` 성공.
  `-o` 관련 컴파일러 경고 2건은 출력 지정에 관한 것으로 전달됐으며 소스 경고 2건으로
  기록하지 않는다. Clippy 성공이나 전체 소스 무경고 판정으로 대신하지 않는다.
- 바이너리 SHA-256: `edb043f4dd1444203c8e8906553083c91f41f86ece1b8af686f5298c1078a81b`.
- 실제 화면: 하네스 `InitialPanel=Timestamp`, 1000×700·920×480 캡처를 메인이 열어 정상 확인.
  - `target/visual-validation/captures/time-001-default-121812.png`
  - `target/visual-validation/captures/time-001-minimum-121814.png`
  OS 전역 입력은 전혀 보내지 않았다. 화면 확인을 실제 OS 클립보드 복사·붙여넣기나
  전역 입력을 통한 UI 왕복 조작 증거로 표현하지 않는다.
- 정리·사용자 환경: 검증 PID `46844` 종료, `sessionRootExists=false`.
  `focusRestore=skipped-focus-changed`로 실제 포커스 복원은 미수행이다.
  사용자 앱 PID `10208`은 그대로 보존했다.
- 최종 보강: 날짜의 소수 9자리 초과를 ISO offset 포함 모든 지원 형식에서 거부하고,
  날짜 범위의 상한에 KST offset을 더할 때 panic 대신 오류를 반환한다. 독립 Code Reviewer
  Noether가 이 수정 후 TIME 동작을 읽기 전용 검토해 PASS 판정했다. Error Reviewer 진단도 추가 오류 없음.
- 보강 후 재실행: `cargo test -p gpui-convenience-tools timestamp --offline -- --nocapture`
  **6 passed**, `cargo test -p gpui-convenience-tools --offline --all-targets --all-features`
  **228 passed / 7 ignored / 0 failed**. 기존 제외 테스트는 새로 성공으로 계산하지 않는다.
  `cargo clippy -p gpui-convenience-tools --offline --all-targets -- -D warnings` 통과.
- 앞의 release 해시·캡처는 보강 전 빌드의 실제 증거다. 보강은 변환 파서·범위 가드만 바꾸었고
  UI는 동일하므로 화면 증거를 재사용했다. 보강 후 바이너리의 새 캡처라고 주장하지 않는다.
- B/T/E/V/R 검증 완료 후 열린 작업에서 이관. `4c368f5`로 커밋·푸시하고 fetch 후
  `HEAD...origin/main`의 ahead/behind `0/0`을 확인했다. 문서 checker 오류 0·작업 ID 14/14.
  구조 지도는 TIME 커밋 범위만 실측하며 미통합 E 파일을 포함한 전체 구조 게이트 통과로 확대하지 않는다.

## 2026-10-21 — 작업 목록 표기 전역 적용·SSL-001 수용 종료

- 대상: 기존 미커밋 작업 트리의 문서만 수정. 제품 소스·사용자 설정·실행 앱은 변경하지 않았다.
- 사용자 요청에 따라 SSL-001을 수용 종료하고 기존 검증 근거를 아래에 보존했다.
  C 미수행 상태를 별도로 남겼으며 커밋·푸시는 실행하지 않았다.
- 전역 정본 `~/.claude/skills/docs-canon/SKILL.md`와 `references/formats.md`에
  ID·제목·진행률·남은 일, 큰 작업의 세부 항목, 완료 이관·Git 반영 구분을 반영했다.
  `~/.codex/AGENTS.md`에는 어댑터 경로만 연결했다. 기존 Claude 지침과 Codex 어댑터는
  같은 전역 정본을 읽으므로 규칙을 복제하지 않았다. 수정 전 전역 파일 3개는 백업했다.
- 이 프로젝트의 열린 ID는 SSL-001만 이관해 13개에서 12개로 바뀌었다.
  나머지 ID·진행률·필수 미검증 조건은 유지하고 남은 일과 확인 주체를 읽기 쉽게 정리했다.
  다른 저장소는 일괄 수정하지 않으며 앞으로 문서 작업 때 그 프로젝트 정본에 맞춰 적용한다.
- 형식 검사: 전역 `check_docs.py . --changed` 1회, 38개 Markdown의 상대 링크·표·등록부
  도달성 오류 0, 앵커·완료 TODO 경고 0. `Assert-ProjectDocs.ps1`은 `active=12 matrix=12`,
  `git diff --check`도 통과했다.
- 스킬 검사: 전역 원본·Codex 어댑터 모두 `python -X utf8 .../quick_validate.py <스킬 폴더>`
  통과. 첫 기본 실행은 Windows cp949 해석 오류였고 UTF-8 모드로 재실행했다.
  검사기 코드는 수정하지 않았으며 규칙·예시는 별도 내용 검토 대상이다.
- Documentation Sync의 읽기 전용 내용 검토에서 E-001의 로컬 보관 조건 누락을 지적받아
  복원했다. SSL-001 수용 종료·기존 증거와 승인 경계·나머지 ID/진행률·전역 이식성은 적합 판정이었다.
- 제품 빌드·Rust 테스트·UI E2E는 문서 전용 변경이므로 미실행이다.

<a id="ssl001-evidence"></a>

## 2026-10-20 — SSL-001 PCF 호환 암복호화

2026-10-21 사용자 확인: “ssl-001은 확인했으니 닫고” 요청으로 **USER_ACCEPTED / 기능 수용 종료**.
구체적인 추가 조작이나 자동 테스트 재실행을 수행한 것으로 확대하지 않는다.
기존 B/T/E/V/R 근거는 아래에 보존하고 열린 TODO·매트릭스에서 이관했다.
**당시 C: 미수행(커밋·푸시 별도 승인 없음)**이며 수용 종료 요청을 외부 작업 승인으로 보지 않는다.
이후 Git 반영은 [2026-10-21 후속 기록](#ssl001-20261021-followup)으로 구분한다.

- 대상: Windows 작업 트리, 기준 HEAD `6e4f1dfa4589f8ed49d9656fdd15ab25b7217df9` 이후
  미커밋 변경. PCF `common/common_util/OpenSslAes.cs`와 `OpenSslPasswordTests.cs`를
  읽기 전용으로 대조했고 PCF 저장소·실운영 암호·설정은 수정하거나 사용하지 않았다.
- 자동 검사: `cargo check -p gpui-convenience-tools --locked`,
  `cargo clippy -p gpui-convenience-tools --all-targets --all-features --locked -- -D warnings`,
  `cargo build -p gpui-convenience-tools --release --locked` 통과.
  신규 Rust 파일 6개는 `rustfmt --edition 2021 --check` 통과; 기존 전체 포맷 차이는 범위 밖이다.
- 관련 테스트: `cargo test -p gpui-convenience-tools openssl --locked -- --nocapture`
  **20 passed / 0 failed**. SHA-256·MD5 PCF fixture와 고정 salt 암호문 일치, 무작위 salt,
  UTF-8·개행·잘못된 암호/헤더/padding·크기 제한, 구버전 설정 기본값을 확인했다.
- 전체 회귀: `cargo test -p gpui-convenience-tools --all-targets --all-features --locked`
  **223 passed / 0 failed / 7 ignored**. 제외 테스트를 실행 통과로 계산하지 않았다.
- 표준 `scripts/Verify-Workspace.ps1`도 필수 GPUI 테스트 41개 및 전체 테스트를 재실행해
  `IDE_VERIFIED`를 출력했다. 스크립트의 `DESKTOP_PENDING`은 IDE 전용 실행의 경계이며
  이번 별도 실제 화면 증거는 아래에 기록한다.
- GPUI 수용 테스트는 다음 실제 뷰 이벤트·상태를 단언한다. OS 클립보드 직접 조작이 아니라
  GPUI 테스트 플랫폼의 클립보드를 사용했다.
  - `openssl_buttons_transform_validate_and_clear_text`: 키 입력 → 인코딩 → Ctrl+A/C 복사 →
    Ctrl+V 붙여넣기 → 디코딩, 오류 시 이전 결과 제거, 지우기 뒤 Ctrl+Z/Y로 복원되지 않음.
  - `openssl_settings_autosave_and_rehydrate_inputs`: 암호 문구·LF 변경 자동 저장,
    다른 설정 보존, 전체 저장, 손상 설정 저장 실패 → 원문 보존 → 복구 후 재시도,
    설정 재로드로 새 뷰 복원. 작업 텍스트는 재로드되지 않음.
  - `openssl_page_contains_inputs_and_scrolls_at_supported_sizes`: 920×480·1000×700·1500×900의
    입력 폭·줄바꿈 레이블·스크롤 후 결과 접근을 단언.
- 실제 release 화면: 검증 초기 패널 `OpenSsl`을 주입해 1000×700·920×480 창을 캡처했다.
  입력·설정·버튼의 수평 영역 침범 없이 표시되고 최소 높이에서는 한 페이지로 스크롤한다.
  최종 바이너리 SHA-256은
  `249dc046602ca58148f482f66216593fb738215690d0f0451a188f8de2c1697c`다.
  - 기본: `target/visual-validation/captures/ssl-001-final-default-162342.png`
  - 최소: `target/visual-validation/captures/ssl-001-final-minimum-162357.png`
  - 초기 캡처에서 발견한 LF 레이블 세로 눌림을 실제 수정하고 재캡처했다.
    초기 테스트의 multiline validator/Root 구성 오류도 수정 후 위 전체 테스트를 재실행했다.
- 격리·복원: 승인받은 사용자 앱 PID 9532만 종료해 빌드했다. 별도 임시 데이터 루트의
  최종 검증 PID 4908을 종료하고 해당 임시 루트 제거를 확인했다. 사용자 데이터로 테스트하지
  않았고 기존 설정을 사용하는 정상 앱을 새 release로 다시 실행했다(PID 36076).
  최종 Start의 포커스 복원은 `failed`, Stop은 `skipped-focus-changed`였다. 전역 마우스·키
  입력은 보내지 않았으며 정확한 포커스 복원을 보장한 것으로 기록하지 않는다.
- 독립 검토: Code Reviewer의 undo 이력·저장 오류 상태 지적 2건을 수정하고 회귀 테스트를
  추가한 뒤 재검토 **PASS**. Error Reviewer는 **코드 오류 없음**으로 판정했다.
  리뷰어의 판단 범위는 읽기 전용 코드·테스트 검토이며 직접 실행한 UI 검증으로 표현하지 않는다.
- 문서 형식: 전역 docs-canon `check_docs.py . --changed` 1회 통과. 38개 Markdown을
  검사해 상대 링크·표·등록부 도달성·앵커 경고 0이었다(자동 선택 진입점 `AGENTS.md`).
  TODO 대응은 `active=13 matrix=13`, 구조는 76파일·26,824줄·최대 944줄·경고 6개로
  통과했다. 문서 역할·설명·명시 앵커는 별도로 검토했고 `git diff --check`도 통과했다.
- 경계: 실제 PCF 서비스 접속·운영 데이터 처리·macOS 실행·실제 OS 클립보드 조작·민감정보
  메모리 zeroization은 검증하지 않았다. CBC는 인증 없는 기존 호환 방식이며 저장된 암호 문구는
  평문이다. 기존 dirty 문서 변경을 보존했고 커밋·푸시는 사용자 승인 대기다.

## 2026-10-19 — docs-canon 적용 검증

- 대상: `6e4f1df` 이후 문서·에이전트 어댑터 작업 트리. Windows, 문서 전용 검증.
- 형식: 전역 `check_docs.py . --registry docs/README.md`와 `--changed` 모두 종료 0.
  Markdown 38개, 상대 파일 링크·표·등록부 도달성 오류 0, 긴 줄 경고 0.
  `git diff --check` 통과. Git의 LF→CRLF 안내는 형식 오류가 아니다.
- 대응·구조: `Assert-ProjectDocs.ps1`은 `active=12 matrix=12`,
  `Assert-ProjectStructure.ps1`은 70파일·25,803줄·최대 938줄·경고 6개로 통과.
- 내용: 변경 전 열린 12개 ID·진행률·게이트 동일, MASTER_PLAN 원문 1,426줄 보존.
  검증 원문과 수치·해시·미검증 경계를 이관하고 제목/명시 앵커 참조를 대조했다.
  Code Reviewer가 지적한 정본 규칙·과거 커밋 경로·AD-006 안내 3건을 수정해 재검토 PASS.
- 전역 도구: checker 회귀 테스트 19/19 및 독립 코드 리뷰 통과.
  전역 원본과 어댑터 메타데이터 검증 통과. Claude 로컬 어댑터는 YAML·필드 검증으로
  확인했다. Codex 전용 검증기는 Claude의 `disable-model-invocation` 필드를 지원하지 않는다.
- 미실행: 제품 빌드·Rust 테스트·앱/UI·VDI·실서버 E2E. 제품 소스·사용자 설정은 변경하지 않았다.
  이번 문서 작업은 제품 미완료 ID나 `[C]` 게이트를 닫지 않았으며 커밋·푸시는 수행하지 않았다.

## 이관 전 품질 게이트 메모 — 과거 실행

아래는 2026-10-19 이관 전 진입부의 원문이다. 서로 다른 회차의 수치와 당시 '최신' 표현을
보존하며 현재 소스 재실행 결과로 사용하지 않는다. 현재 실행법은 VERIFICATION.md가 정본이다.

최신 품질 게이트 기준은 `cargo check -p gpui-convenience-tools --locked`, 전체 테스트
186 passed·5 ignored, 표준 `cargo clippy -p gpui-convenience-tools --all-targets --all-features --locked -- -D warnings` exit 0·경고
0건이다. `scripts/Verify-Workspace.ps1`도 IDE 안전 모드에서 필수 GPUI 테스트 34개와 전체
테스트를 재실행해 `IDE_VERIFIED`를 출력했다. 실제 데스크톱 표면은 별도 포그라운드 조건이
필요하므로 같은 실행에서 `DESKTOP_PENDING`으로 분리했다. 전체 `cargo fmt --check`는 기존
baseline 포맷 차이가 남아 있어 별도 구조 정리 범위로 유지한다.
이번 실행에서는 `DOCS_VERIFIED active=23 matrix=23`도 먼저 통과해 활성 TODO와 검증
매트릭스의 일대일 대응을 확인했다.

AD-006을 포함한 이전 변경 코드의 검증 결과는 전체 199 passed·7 ignored 및 Clippy `-D warnings`
통과다. 위 `IDE_VERIFIED`와 `DOCS_VERIFIED`는 이전 기준 실행 결과이므로 이 수치의
재실행 증거로 간주하지 않는다.
2026-10-12 탐색 작업 화면의 최종 결과는 전체 203 passed·0 failed·7 ignored이며,
해시별 실제 화면 증거와 이전 결과의 구분은 아래 수용 완료 기록을 참조한다.

## 2026-09-29 Phase O 격리 E2E 보강

Windows Codex의 `CLAUDE_LOCAL` 하네스를 사용했다. 사용자 앱 PID 44832는 그대로 두고,
실행 중임을 읽기 전용 상태 조회로 확인한 `TACS` VM의 VDI는 열지 않았다. 이미 있는
VirtualBox 생성 5 MiB 합성 NTFS VDI를 `-SeedVdi`로 작업별 임시 루트에 복사했다.
릴리즈 바이너리 SHA-256은 `67D9F1F6F219F2327427D3447A98B8E9EAE7D11B09B66202257480987667F80F`다.
`cargo test -p gpui-convenience-tools --locked virtual_disk` 결과는 81 passed·3 ignored였다.

- `VDE-024`·`VDE-019`: 1200×900 초기 캡처 `phase-o-offline-initial-20260929-160012.png`에서
  루트 트리·파일 목록과 숨김/시스템 폴더를 확인했다. `-InitialGuestPath '$Extend'` 캡처
  `phase-o-subfolder-20260929-160715.png`에서는 현재 경로가 `$Extend`로 바뀌고 그 자식
  `$ObjId`가 표시됐다. 이는 시작 상태를 주입한 E2E이며 실제 트리 클릭 증거는 아니다.
- `VDE-012`·`VDE-015`·`VDE-019`: `-AutoCopyVdi` 격리 릴리즈 캡처
  `phase-o-offline-autocopy-20260929-160303.png`에 `완료 · 파일 17개 · 915.3 KB · 실패 1`과
  손상 `many_subdirs` 사유가 표시됐다. 격리 대상 실제 파일은 17개·937,234바이트였고
  13개가 숨김 또는 시스템 속성이었다. 실제 오류 억제 버튼 클릭·자연 속도 대용량 중지
  조작은 수행하지 않았다.
- `VDE-017`: 합성 미지원 파티션 VDI 캡처
  `phase-o-unsupported-20260929-160842.png`에서 NTFS 3.1만 지원한다는 오류·파티션
  경고를 확인했다. 실행 중 VM 판정은 자동 테스트
  `detects_a_running_vm_disk_from_machine_readable_info`로만 확인했고 실제 VM의 VDI를
  열어 보는 E2E는 사용자 결정에 따라 적용하지 않는다.

각 `Start`의 `launchFocusRestore`는 `skipped-focus-changed`였으므로 전역 Click/Key/Drag는
보내지 않았다. 따라서 `VDE-012`·`VDE-014`·`VDE-016`·`VDE-023`·`VDE-025`의 실제 입력
게이트는 그대로 남는다. 네 구현 검증 세션은 각각 `Stop` 후 임시 루트 제거를 확인했다.

독립 Visual Reviewer도 같은 SHA-256 릴리즈를 다른 격리 세션으로 시작해 GPUI 관련
테스트 70 passed·3 ignored를 재실행했다. 자체 첫 캡처
`independent-vde019-root-first-1200x900-161356.png`에서 NTFS 3.1·숨김/시스템 항목과
트리/목록 경계, `independent-vde019-extend-1200x900-161439.png`에서 `$Extend` 자식
`$ObjId`, `independent-vde019-autocopy-1200x900-161718.png`에서 복사 17개·실패 1건을
독립적으로 재확인했다. 대상 실제 파일은 17개·937,234바이트였고 구현 검증과 불일치는 없었다.
그러나 세션의 `launchFocusRestore: failed`와 사용자 작업 상태 불확실성 때문에 전역 입력을
보내지 않았으므로 종합 판정은 `BLOCKED(input)`이다. 실제 클릭·키보드·버튼 동작의 독립
검증을 `PASS`로 승격하지 않는다. 검토자의 세 세션은 모두 `Stop`으로 정리했고 사용자 앱
PID 44832는 유지됐다.

## 2026-09-30 필수 검증 기준 재판정

사용자 요청에 따라 공통 완료 기준을 실용적으로 조정했다. 과거의 실제 외부 입력·독립
재조작 미수행 기록은 그대로 보존하며, 새 검증을 수행했다고 주장하지 않는다.
축약 전 상세 증거는 커밋 `1f5320f`의 이 문서에 보존한다.

이관 주: 위 '이 문서'의 당시 경로는 `docs/VERIFICATION.md`다.
`git show 1f5320f:docs/VERIFICATION.md`로 조회한다.

기존 이벤트·상태 테스트와 실제 릴리즈 증거로 `VDE-012`·`VDE-024`·`VDE-022`·`VDE-019`를
완료 기록으로 옮겼다. Code Reviewer가 실제 단언을 확인했고, `VDE-017`의 실행 중 VM 가드는
경로 일치뿐 아니라 열기 거부·소스 미연결을 확인해야 하므로 계속 활성으로 유지했다.
`VDE-014`·`VDE-015`·`VDE-016`·`VDE-025`의 버튼/수정키/저장 연결 단언과 `VDE-023`의 OS
대화상자 선택·취소는 남는다. 테스트 추가·실제 UI 재실행은 이번 문서 변경의 범위가 아니다.
별도 구조 항목 `G-001`도 기존 bounds 테스트와 릴리즈 두 화면 증거로 종료했다.
`C-002`는 기존 사용자 프로필 확인만, `G-003`은 드래그 저장·재로드 단언만 남긴다.

이번 문서 전용 변경은 `Assert-ProjectDocs.ps1`의 `DOCS_VERIFIED active=17 matrix=17`,
에이전트 TOML 3개 파싱, 정본·절차 참조 경로 확인, `git diff --check`를 통과했다.
제품 코드·설정·실행 중 앱은 변경하지 않았고 코드 테스트·새 화면 E2E는 실행하지 않았다.

## 2026-09-30 Phase O 수용 범위와 O-5 실제 검증

사용자는 현재 보이는 영역의 정상 표시를 우선하며 추가 입력 테스트 대부분의 필요성을
느끼지 않는다고 명시했다. `VDE-014`·`VDE-015`·`VDE-016`·`VDE-025`의 추가 조합·버튼
연결·대상 폴더 대화상자·단축키·드래그 저장 단언은 **선택 보강으로 전환**했다.
기존 기능과 저장 계약·복사 엔진의 취소 및 부분 파일 정리 테스트는 그대로 유지한다.
이 전환은 미수행 테스트의 `PASS`나 사용자 직접 조작 `USER_ACCEPTED`가 아니다.
이전 필수 조건과 증거는 커밋 `74b4a49`의 이 문서에 보존한다.

이관 주: 위 '이 문서'의 당시 경로는 `docs/VERIFICATION.md`다.
`git show 74b4a49:docs/VERIFICATION.md`로 조회한다.

### 안전 E2E — VDE-017 PASS

앱 제품 코드는 바꾸지 않고 기존 `target/release/gpui-convenience-tools.exe`를 사용했다.
SHA-256은 `67D9F1F6F219F2327427D3447A98B8E9EAE7D11B09B66202257480987667F80F`다.
`GPUI_CONVENIENCE_TOOLS_DATA_DIR`에 격리하고 광고·자동 동기화를 끈 뒤 합성 디스크와
테스트 전용 VBoxManage 대역으로 실제 릴리즈의 시작→조회 프로세스→거부 화면을 검증했다.
실제 실행 중인 TACS 디스크를 여는 시도는 하지 않았다.

- 실행 중 VM 응답: `list runningvms`→`showvminfo ... --machinereadable` 2회 호출,
  실행 중 사용 안내·미연결 헤더·파티션 없음 확인. PID 33900.
  증거: `target/visual-validation/o5-running-2008c24274864cba95ef091b7125d782/running.png`.
- 조회 실패: 대역 exit 7·`O5_PROBE_QUERY_FAILURE`, 조회 실패 차단과 파티션 없음 확인.
  PID 21840. 증거:
  `target/visual-validation/o5-queryfailure-5bbbe26044534c1b80eaf753d1f31318/query-failure.png`.
- 잠금 표식: `fixture.vdi.lck`에서 조회 호출 0회, 잠금 사유·미연결·파티션 없음 확인.
  PID 17748. 증거:
  `target/visual-validation/o5-locked-bf2f85e5929d4b8ab81c828eb550a4c0/locked.png`.
- 각 세션에서 원본 SHA-256·길이·수정시각 불변과 임시 대상 빈 상태를 확인했다.
  `Assert`는 프로세스 조회·파일 불변만 단언하며 앱의 거부·미연결 판정은 위 실제 화면으로
  확인했다. 내부 `source=None` 값을 테스트 API로 직접 단언한 것은 아니다.
  초기 세션은 대상 입력과 임시 폴더가 연결되지 않았으므로 빈 폴더만으로 복사 차단을
  증명하지 않는다. 이 연결은 후속 하네스 리뷰에서 보강했다.

초기 하네스의 file log 단언은 실패했다. `push_log`가 이 메시지를 UI 로그에만 넣는 것을
확인하고, 파일 로그를 근거로 앱 상태를 단언하지 않도록 수정했다. 조회 trace 1행의
PowerShell scalar/array 차이도 수정한 뒤 조회 실패 시나리오 assertion을 다시 통과했다.
후속 리뷰에서 하네스의 임의 이미지 입력을 제거하고 동봉 NTFS fixture 생성으로 고정,
reparse-point 차단·동일 Process 핸들 종료·manifest 저장 실패 정리·앱 복사 대상 연결을
보강했다. 보강 후 `Prepare`의 fixture export·대역 컴파일은 통과했지만, 사용자 입력
감지 후 UI 실행을 중단했으므로 보강된 Start/Stop을 새 실제 앱으로 재실행한 것은 아니다.

### 파일 선택 — VDE-023 PENDING(input)

격리 PID 21100의 찾아보기 버튼으로 실제 네이티브 열기 대화상자와 파일 이름·선택·취소
컨트롤 표시를 확인했다. 입력 도구가 부모 앱과 소유 대화상자의 대상 좌표를 일치시키지
못했다. `set_value`는 `Cannot set a value for an element that is not settable`,
취소 클릭은 `point ... is over ... "탐색 단추", not target window ...`로 실패했다.
경로 선택과 취소 성공은 주장하지 않는다. 남은 확인은 종료된 VM의 VDI 선택→경로 반영,
다시 찾아보기→취소→동일 경로 유지 각 1회다. 제품 결함으로 재현된 것은 아니다.

커서 복원 API는 제공되지 않았다. 이전에 관찰한 탐색기 포커스 복원 시도도
`user input was detected in this window; call get_window_state before continuing`로
거부되어 추가 UI 조작을 중단했다. 포커스·커서 복원 성공으로 기록하지 않는다.
4개 검증 PID는 모두 종료했고 사용자 앱 PID 8488은 유지했다. 증거 루트는 삭제하지 않았다.

### 재실행과 자동 검증

```powershell
# Prepare는 fixture·대역만 만들며 앱 UI를 실행하지 않는다.
pwsh -NoProfile -File scripts/Test-VdiSafetyE2E.ps1 -Action Prepare
# 아래 Start는 실제 검증 창을 띄운다. 출력 sessionPath로 Assert/Stop을 수행한다.
pwsh -NoProfile -File scripts/Test-VdiSafetyE2E.ps1 -Action Start -Scenario Running
pwsh -NoProfile -File scripts/Test-VdiSafetyE2E.ps1 -Action Assert -SessionPath <session.json>
pwsh -NoProfile -File scripts/Test-VdiSafetyE2E.ps1 -Action Stop -SessionPath <session.json>
```

`QueryFailure`·`Locked`도 같은 절차로 실행한다. 실제 화면의 거부/미연결 확인을 생략하고
`O5_PROBE_ASSERT_PASSED`만으로 종합 PASS를 내지 않는다.
`cargo test -p gpui-convenience-tools virtual_disk --locked`는 **81 passed·3 ignored**,
`Prepare`의 동봉 fixture export는 **1 passed**다. ignored는 실제 이미지 주입용이므로
실행 중 VM 파일을 주입해 해제하지 않았다. 제품 소스 구조는 67파일·25,452줄로 변경 없다.

`cargo check -p gpui-convenience-tools --locked`, PowerShell parser(오류 0),
테스트 대역 `rustc --edition=2021 -D warnings`·`rustfmt --check`, 문서 assertion
`DOCS_VERIFIED active=12 matrix=12`, 구조 assertion과 `git diff --check`를 통과했다.
새 제품 코드 변경이 없어 사용자 앱 종료·릴리즈 재빌드는 수행하지 않았다.

읽기 전용 Code Reviewer의 안전 코드·문서 정합성 검토를 통과했다. 검증 도구·증거 및
선택 범위 변경은 커밋 `f91dc66`으로 `origin/main`까지 푸시했고 로컬/원격 해시 일치를
확인한 뒤 아래 C 게이트를 체크했다. 리뷰어의 새 실제 UI 재현을 주장하지 않는다.

<a id="vde-026-workspace-evidence"></a>

## 2026-10-12 탐색 작업 화면 — 구현·검증·커밋 완료

대상은 기준 HEAD `37c6353` 위의 VDE-026 작업 트리다. Windows 로컬에서 구현/검증
담당자가 전달한 최종 결과와 실제 캡처를 기록한다. 문서 담당자는 소스·테스트 단언과
캡처 파일을 읽었으며 제품 테스트·앱 조작을 별도로 재실행하지 않았다.
사용자 흐름은 [README.md](README.md#virtualbox-workspace), 최종 실측은
[PROJECT_MAP.md](PROJECT_MAP.md)를 정본으로 한다.

### 자동 검증·리뷰

구현·검증 담당자 확인 결과: 관련 테스트 **85 passed·0 failed·3 ignored**, 전체 테스트
**203 passed·0 failed·7 ignored**. `cargo check`, Clippy
`--all-targets --all-features --locked -- -D warnings`(경고 0), suffix 릴리즈 빌드,
변경 파일 rustfmt 검사 모두 통과했다. 코드 reviewer의 수정 후 소스 재리뷰도 통과했고
추가 finding은 없었다. 이전 전체 202 passed 결과는 reviewer 수정 전 중간 결과이므로
최종 수치로 사용하지 않는다. ignored는 실행 성공으로 세지 않는다.

첫 테스트 실패 뒤 결과 상세는 footer 일반 자식에서 `absolute` 보조 오버레이로 수정했다.
`window/virtual_disk/copy.rs::render_details`의 현재 배치는 footer 기준 `bottom(76px)`·
높이 `112px`이며 목록 할당 높이를 줄이지 않고 겹쳐 표시한다. 상세 내부에는 별도 스크롤을 둔다.
GPUI 0.2.2 `Frame.clear`의 stale selector bounds 캐시 문제는 구현·검증 담당자가 확인한 진단이다.
접은 뒤의 부재 확인만 표시 state 단언으로 전환했으며 최초 렌더 부재·공간 배분·스크롤·
실제 클릭 후 상태 단언은 유지한다. 문서 담당자의 독립 캐시 재현으로 표시하지 않는다.

- 연결 전 설정 단계: `virtual_disk_panel_registers_navigation_and_renders_read_only_shell`로
  빈 탐색기·복사줄 미표시를 확인했다.
- 탐색 공간·footer: `virtual_disk_workspace_prioritizes_files_at_all_supported_sizes`와
  `virtual_disk_explorer_keeps_tree_and_file_list_inside_workspace`의 920×480·1000×700·
  1600×1000 bounds, 최종 1000×700 릴리즈 기본 화면을 확인했다.
- 설정 왕복·선택 보존·열 너비 접기: `virtual_disk_workspace_returns_to_setup_without_losing_selection`
  통과. 실제 단계 왕복은 아래 이전 후보 해시의 증거로 구분한다.
- 폭 조절·저장: `virtual_disk_layout_controls_update_and_reset_readable_column_widths`와
  `virtual_disk_tree_width_setting_is_clamped_and_persisted` 통과. 기존 저장 계약은 유지한다.
- 결과 상세: `virtual_disk_workspace_error_and_copy_details_remain_accessible`과
  `virtual_disk_panel_renders_copy_progress_and_issue_summary`로 접힘·열기/닫기 state·목록
  최소 높이·마지막 오류 스크롤 접근을 확인했다. 동일 높이의 직접 비교 단언으로 확대하지 않는다.
- 복사 중 변경 차단·긴 오류 스크롤·중지 접근:
  `virtual_disk_workspace_keeps_stop_visible_with_long_error_while_copying`으로 최대 48px
  오류 영역·스크롤 offset·목록 공간·중지 버튼 경계·변경 클릭 차단·중지 상태를 확인했다.

workspace 테스트 4개와 변경된 목록 경계 테스트는 `scripts/Verify-Workspace.ps1` 필수
목록에 등록되어 있다. 이 등록 사실 자체를 별도 실행 성공으로 세지 않는다.
최종 문서·구조 검사도 통과했다: `DOCS_VERIFIED active=12 matrix=12`,
`STRUCTURE_VERIFIED files=70 lines=25803 max=938 warnings=6`. `git diff --check`도 통과했다.

### 실제 릴리즈 증거 — 해시별 구분

캡처 루트는 `target/visual-validation/vde026-c7adcc6baa9f416f90095b4d7a57c2a2/`다.

- 이전 후보 SHA-256 `0EF177B5E235D2DA9E21FA6F795D85930CA926554FA08B77B93FF72647B06782`,
  PID 25516, 1000×700: `explorer-default.png`·`setup-return.png`·`explorer-resumed.png`.
  검증 담당자가 native 입력으로 활성화→디스크 변경 클릭→돌아가기 클릭을 수행해
  연결 설정 카드 숨김·탐색 영역 우선 배분·하단 copy footer와 단계 왕복을 확인했다.
  문서 담당자는 캡처 3개를 읽었다. 이후 reviewer 수정본의 실제 왕복 재현이라고 표시하지 않는다.
- 최종 SHA-256 `5848B79C54915469C884629DBD72228A7B0AC730877C71E0D5F9CA1E2C28101D`,
  PID 44472, 1000×700: `final-explorer.png`. reviewer 수정 후 suffix 릴리즈의 기본 탐색
  화면을 실제로 확인해 레이아웃 수용 통과가 전달되었고 문서 담당자도 저장 캡처를 확인했다.
  이 캡처는 기본 상태이며 복사 중 변경 차단·긴 오류·상세 스크롤의 실제 재조작 증거로 확대하지 않는다.
- 검증은 원본 합성 fixture만 사용했다. SHA-256
  `F642F04D87CBE4CCF671D9FEB78E2DEC77B7962766645C10CC0DF73BA478EB78` 전후 동일,
  대상 파일 0개가 전달되었다. 실제 사용자 디스크나 실행 중 VM 파일은 열지 않았다.
- 각 검증 PID 25516·44472만 확인 후 종료했고 사용자 PID 10856은 유지했다.
  기존 사용자 앱·시스템 설정은 변경하지 않았다. 커서/포커스 복원 API가 없어 복원 성공을 주장하지 않는다.
- 최종 바이너리는 사용자 실행 중인 기본 exe를 교체하지 않고 `target/release/deps`의
  suffix 바이너리로 빌드했다. 기본 앱 갱신은 별도 사용자 앱 종료 후 빌드가 필요하다.

2026-10-12 요청 범위의 구현·검증 완료에 따라 활성 TODO를 닫고 완료 검증 행으로 옮겼다.
구현 커밋 `7570a64`를 `origin/main`으로 푸시한 뒤 fetch, 로컬/원격 해시 일치,
ahead/behind `0/0`과 clean 상태를 확인해 **C 게이트를 [x]**로 기록한다.
남은 Phase O OS 파일 선택·취소
작업은 그대로 유지한다. 최신 실제 디스크 진단은 이 구현 범위가 아니며 과거 MSR·BitLocker
기록은 당시 조건의 역사로 보존한다.

최신 VDE-019 릴리스 보강 검증은 `exports_ntfs_vdi_fixture_when_requested`로 격리 합성 VDI를
재생성한 뒤 수행했다. 최신 release 앱에서 표준 VBox VDI와 재생성 합성 VDI 모두 파일 행,
숨김·시스템 속성, 복사 결과 17개·915.3KB, `many_subdirs` 실패 사유를 확인했고 캡처는
`continuation-vde019-standard-release-031918.png`,
`continuation-vde019-synthetic-refreshed-032017.png`이다. 오류 억제 버튼 `Click`은
포그라운드 안전 검사에서 차단되어 실제 릴리스 클릭 성공으로 기록하지 않았으며, 세션 종료 후
`PROCESS_COUNT=0`·`SESSION_COUNT=0`을 확인했다.

VDE-017 미지원 파티션 릴리스 검증은 `exports_unsupported_partition_vdi_fixture_when_requested`로
MBR 타입 `0x83`·NTFS 부트 시그니처가 없는 VDI를 격리 생성한 뒤 수행했다. 최신 release 앱에서
파티션 행의 `미지원/미확인 파일시스템` 상태와 `지원하지 않는 파일시스템` 오류 카드를 함께
확인했으며 캡처는 `vde017-unsupported-partition-release-fixed-033450.png`이다. 검증 후
`PROCESS_COUNT=0`·`SESSION_COUNT=0`을 확인했다. fixture가 NTFS로 오인되지 않는지
`reads_unsupported_partition_fixture_without_claiming_ntfs` 단위 테스트도 통과했다.

G-003의 실제 divider 드래그 경로는 검증 하네스에 `-Action Drag -X <start> -Y <start>
-ToX <end> -ToY <end>`를 추가한 뒤 1000×700 release 화면에서 시도했다. 시작 캡처는
`target/visual-validation/captures/g003-drag-before-063747.png`에 남겼지만, 현재 데스크톱의
포그라운드를 검증 창으로 전환하지 못해 하네스가 전역 입력을 보내지 않고 중단했다. 따라서
실제 mouse-up 저장 콜백 성공으로 기록하지 않으며, 세션 정리 후 `PROCESS_COUNT=0`·
`SESSION_COUNT=0`을 확인했다. GPUI `sidebar_divider_drag_resizes_navigation_and_content`
회귀 테스트와 하네스 구문 검증은 통과했다.

## 완료 검증 기록

| ID | 프로필 | B | T | E | V | R | C | 증거 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| VDE-026 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 2026-10-12 전체 203 passed·0 failed·7 ignored, 관련 85 passed·3 ignored, check·Clippy 경고 0·suffix release·변경 파일 rustfmt 및 코드 재리뷰 통과. 최종 1000×700 실제 화면 수용 완료, 단계 왕복은 이전 후보 해시로 구분. [해시별 증거·확인 한계](#vde-026-workspace-evidence) 참조. 구현·검증 완료로 TODO 종료; `7570a64` 푸시 후 fetch·원격 해시 일치·ahead/behind 0/0·clean 확인. |

`VDE-014`·`VDE-015`·`VDE-016`·`VDE-025`는 추가 검증을 선택 범위로 옮긴 기록이다.
해당 행의 E `—`는 이번 수용 범위에서 제외했다는 뜻이며 이전 미검증 사실을 지우지 않는다.

완료 작업은 `TODO.md`에서 제거하고 이곳에 게이트 결과를 보존한다. 상세 설계와 완료 단계는
`MASTER_PLAN.md`에 한 번만 기록한다.

| ID | 프로필 | B | T | E | V | R | C | 증거 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| VDE-017 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 2026-09-30 실행 중 VM 모의 응답·조회 실패·잠금 표식 3개 실제 릴리즈 차단 화면과 원본 불변/조회 trace를 확인. 위 O-5 증거 참조. 실제 실행 중 VM 디스크는 열지 않음. 관련 자동 테스트 81 passed·3 ignored. |
| VDE-025 | E2E | [x] | [x] | — | [x] | [x] | [x] | 컬럼 폭 보정·설정 round-trip·조절 버튼·bounds 테스트와 기존 릴리즈 캡처 통과. `virtual_disk_tree_width_setting_is_clamped_and_persisted`는 setter 호출이며 저장 비활성 상태라 drag 저장의 증거가 아님. 선택 보강: GPUI drag/mouse-up→설정 저장·재로드와 새 화면 폭 복원 단언.  2026-09-30 사용자 요청으로 추가 검증을 필수 범위에서 제외. 미수행을 PASS로 표시하지 않음. |
| VDE-014 | GPUI | [x] | [x] | — | [x] | [x] | [x] | 기존 더블클릭 폴더 진입 GPUI 이벤트·단순/Ctrl 선택과 Shift 선택 헬퍼 테스트 통과. 선택 보강: modifier가 있는 실제 GPUI 행 이벤트의 선택 집합과 상위 버튼 경로·목록 단언. 외부 키보드 반복 검증은 요건에서 제외.  2026-09-30 사용자 요청으로 추가 검증을 필수 범위에서 제외. 미수행을 PASS로 표시하지 않음. |
| VDE-015 | GPUI | [x] | [x] | — | [x] | [x] | [x] | 청크 취소·부분 파일 정리 자동 테스트, 실제 합성 VDI 복사와 제어된 취소 증거는 기존 기록 참조. 선택 보강: 대상 입력→시작/중지 버튼의 작업·취소 상태 자동 단언 및 대상 폴더 대화상자 선택/취소 1회. 자연 속도 초대형 파일 검증은 불필요.  2026-09-30 사용자 요청으로 추가 검증을 필수 범위에서 제외. 미수행을 PASS로 표시하지 않음. |
| VDE-016 | GPUI | [x] | [x] | — | [x] | [x] | [x] | Ctrl+A 선택은 GPUI 결과 단언으로 검증됨. 기존 종합 단축키 테스트는 빈 화면의 요소 존재만 확인한다. 선택 보강: 로드된 목록의 Enter/Backspace/F5/Ctrl+C 결과 및 입력창 포커스 시 비작동 단언.  2026-09-30 사용자 요청으로 추가 검증을 필수 범위에서 제외. 미수행을 PASS로 표시하지 않음. |
| G-001 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `ButtonStyle` 기본값 통일 및 개별 덮어쓰기 제거; 검증 전용 `-InitialPanel`로 패널 전환 입력을 격리 실행에 주입; `cargo check -p gpui-convenience-tools --locked`; 관련 GPUI 테스트 `ad_block_cards_contain_long_content_at_supported_widths`, `service_rows_keep_names_readable_at_supported_window_widths`, `file_sync_sections_share_one_width_at_every_window_width`; 전체 테스트 145 passed·4 ignored; Clippy exit 0(기존 경고 7건); 격리 release 파일 동기화 캡처 `target/visual-validation/captures/g001-file-sync-013019.png`와 자동 시작 캡처 `target/visual-validation/captures/g001-auto-start-013004.png`에서 두 화면과 공용 버튼 스타일 확인, processCount=0·sessionCount=0; 2026-09-30 기존 bounds·릴리즈 화면 증거와 읽기 전용 코드 리뷰로 완료 재판정(새 UI 실행 아님); commits `0d7bf25` 및 후속 검증 경로 커밋 push 완료 |
| VDE-024 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 지연 로딩 `GuestDirectoryTreeNode`와 좌우 `balanced_split` 탐색 영역을 추가해 폴더 트리 선택으로 중첩 경로를 바로 열고, 목록 행 높이·열 폭·탐색 카드 높이를 컴팩트하게 조정; `virtual_disk_folder_tree_navigates_nested_paths_without_repeated_list_clicks`, `virtual_disk_explorer_keeps_tree_and_file_list_inside_compact_card`와 전체 169 passed·4 ignored, Clippy `-D warnings` 통과; `Verify-Workspace.ps1` 필수 GPUI 테스트 29개 이름 검증; release 캡처 `target/visual-validation/captures/vde024-tree-920x700-121233.png`, `vde024-tree-1000x700-121200.png`, `vde024-tree-1280x900-121234.png`; 실제 트리 Click은 하네스 좌표 변환으로 상태 전환을 확정하지 않아 GPUI 이벤트 테스트로 대체, commit `4af7cc6` push 완료; 2026-09-30 새 기준으로 완료: 실제 GPUI 트리 클릭→직계 경로/목록 단언과 기존 캡처가 충분 |
| VDE-022 | RUST | [x] | [x] | [x] | [x] | [x] | [x] | 실제 `D:\VMMachine\win11\TACS\TACS_1.vdi`를 읽기 전용으로 대조해 `VDI normal (base)`, `dynamic default`, GPT 1번 MSR, 2번 `-FVE-FS-` BitLocker를 확인; `classifies_bitlocker_and_microsoft_reserved_partitions_explicitly`, `encrypted_partition_error_explains_the_offline_boundary`, 전체 테스트 163 passed·4 ignored, release 캡처 `target/visual-validation/captures/vde022-final-095347.png`에서 MSR·BitLocker 라벨과 안내를 확인; commit `6b89a09` push 완료; 2026-09-30 새 기준으로 완료: 분류·안내 자동 테스트와 기존 릴리즈 화면 확인이 충분 |
| VDE-012 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 160 passed·4 ignored; `virtual_disk::copy::tests::collecting_copy_errors_removes_partial_output_and_keeps_issue`로 원본 단축 읽기 실패 뒤 부분 대상 파일 제거와 원본 `SourceChanged` 사유 보존을 확인하고, 오류 행 selector·억제 키 저장/재로드·미억제 토스트 게이트와 `virtual_disk_panel_renders_copy_progress_and_issue_summary`의 GPUI `simulate_click` 억제/재표시 dispatch를 검증; 실제 VBox 생성 VDI 릴리스 복사에서 `many_subdirs` 손상 사유·17개 파일 부분 결과를 확인; 2026-09-30 새 기준으로 완료: GPUI 억제/재표시 클릭·저장 단언과 릴리즈 오류 표시 증거가 충분하며 별도 외부 클릭은 선택 |
| VDE-019 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 160 passed·4 ignored, VDI 집중 60 passed·2 ignored; 표준 VDI 헤더 오프셋 수정 후 VirtualBox 7.2.14 `convertfromraw` 생성 VDI(`Storage format: VDI`, `State: created`)를 `GPUI_CONVENIENCE_TOOLS_VBOXMANAGE` 환경 변수와 함께 격리 릴리스 앱에 주입; 숨김/시스템 항목 17개·937,234바이트 복사와 `many_subdirs` 손상 사유 확인; 최신 release 1200×1000 캡처 `target/visual-validation/captures/continuation-vde019-current-release-040648.png`에서 숨김/시스템 행과 17개·915.3KB·실패 1건 요약을 재확인; 이번 구현 세션에서도 1200×1000 `target/visual-validation/captures/continuation-current-vde019-1200x1000-052717.png`와 920×700 `target/visual-validation/captures/continuation-current-vde019-920x700-052734.png`를 캡처해 안전 경계·NTFS 3.1·숨김/시스템 행·16개 선택과 compact 폭 경계를 확인; 선택 전 `0개 선택` 캡처와 `-SelectAllVdi` 후 `16개 선택` 캡처 `target/visual-validation/captures/vde019-vbox-key-before-024822.png`, `vde019-vbox-select-all-seed-025058.png`; 기본 포커스 전환·UI Automation `SetFocus`·`SetWindowPos` 모두 foreground를 바꾸지 못해 `Key Ctrl+A`는 안전 차단으로 미검증; 세션 종료 후 `PROCESS_COUNT=0`·`SESSION_COUNT=0`·대상 루트 삭제; 2026-09-29 격리 복사 17개·937,234바이트 및 독립 증거는 위 기록 참조. 2026-09-30 목록·호스트 복사 통합 범위만 완료로 재판정; 선택·단축키·복사 버튼·안전 가드의 잔여 단언은 VDE-014~017에서 추적; commits `87937dc`, `bed5914` |
| AD-006 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `kakao_layout.rs`의 창 계층·크기 지문 및 6개 HWND 적용/역순 복원, GPUI 경계 테스트와 전체 199 passed·7 ignored, 릴리즈 빌드·Clippy `-D warnings` 통과(기존 기록). 실제 KakaoTalk 전후 캡처 `target/visual-validation/captures/ad006-before.png`·`target/visual-validation/captures/ad006-reclaimed.png`와 사용자 실기기 확인을 근거로 Phase A 종료. 독립 Visual Reviewer 세션은 수행하지 않았고, 이번 사용자 직접 수용을 V 게이트의 명시적 예외로 기록한다. 새 데스크톱 테스트를 수행했다는 뜻이 아니다. |
| VDE-021 | DECISION | — | — | — | — | [x] | [x] | 2026-09-29 사용자 결정: 실행 중 VM 파일 접근은 VDI 직접 읽기와 `VBoxManage guestcontrol` 모두 구현하지 않는다. O-6을 종료했고, 과거 분리 설계(commit `28dc516`)는 `VIRTUAL_DISK_BACKENDS.md`에 미채택 이력으로 보존한다. 실행 중 VM 파일을 검증용으로 열거나 Guest Control E2E를 수행하지 않았다. |
| C-001 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 실행파일 옆 `settings.json` 우선 경로·구버전 AppData `config.json` fallback·`GPUI_CONVENIENCE_TOOLS_DATA_DIR` 격리 경계와 `VirtualDiskConfig` 왕복을 구현; 정상 실행 경로가 실행파일 옆 `settings.json`을 가리키는 회귀 테스트와 설정 테스트 전용 환경 잠금 추가; GPUI `settings_page_keeps_user_config_actions_inside_the_card` 통과; 설정 화면에 현재 경로·`설정 저장`·`파일 위치 열기`를 추가했고 창 닫기/Drop 저장 경계를 연결; 전체 167 passed·4 ignored, Clippy `-D warnings` 통과, `DOCS_VERIFIED active=28 matrix=28`, `STRUCTURE_VERIFIED files=56 lines=22003 max=920 path=app/src/config.rs warnings=4`; release 설정 화면 1000×700 `target/visual-validation/captures/c001-settings-1000x700-111818.png`, 920×700 `c001-settings-920x700-111840.png`, VDI 입력 복원 `c001-vdi-restored-1000x700-112034.png`; commit `1d7b972` push 완료 |
| AD-001 | RUST | [x] | [x] | — | — | [x] | [x] | `platform::windows::window_ops::tests` 3개 통과; `cargo check --locked` 통과; 최상위 소유/도구 팝업만 후보화, 상태 저장·0×0 동작은 후속 AD-002·AD-003 |
| AD-002 | RUST | [x] | [x] | — | — | [x] | [x] | `window_ops::tests` 5개 통과; `cargo check --locked`; `Verify-AdWindowState.ps1 -ProcessId 26440`로 KakaoTalk/WebView2 관련 PID·창 상태를 읽기 전용 확인; 실제 사용자 창 조작은 수행하지 않음 |
| AD-003 | RUST | [x] | [x] | — | — | [x] | [x] | `window_ops::tests` 7개 통과(전용 0×0 fixture 포함); `cargo check --locked`; `SetWindowPos` 0×0·`EnableWindow(FALSE)`·원상 복원 검증; 사용자 PID 26440은 읽기 전용 상태 확인만 수행 |
| AD-004 | RUST | [x] | [x] | — | — | [x] | [x] | `window_ops::tests` 9개 통과(다중 tool-window fixture·무효 HWND 경계 포함); 다중 HWND 추적·닫힌/재생성 창 정리·프로세스 ID 재사용 방어·수동 상태 변경 재축소·창별 복원 실패 로그 구현; 전체 테스트 및 `Verify-Workspace.ps1` 통과 |
| AD-005 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | ToolHelp 프로세스 트리·`Chrome_WidgetWin_1` 자식 후보 탐색 구현; 사용자 제공 실제 KakaoTalk 광고영역 0×0 확인으로 닫음; 구현 세션의 격리 릴리즈 캡처는 `target/visual-validation/captures/ad-panel-contained-rows-final-2-154739.png`; 커밋 `a9f99d7` 푸시 완료 |
| UI-001 | GPUI | [x] | [x] | [x] | [x] | [x] | [x] | `ad_block_cards_contain_long_content_at_supported_widths`가 920/994/1000/1280px에서 카드·행·클래스 열 경계 통과; 릴리즈 `CLAUDE_LOCAL` 1085×804 캡처 `target/visual-validation/captures/ad-panel-contained-rows-final-2-154739.png`에서 `KakaoTalk.exe`·`Chrome_WidgetWin_1` 표시와 카드 경계 확인; 커밋 `a9f99d7` 푸시 완료 |
| VDE-003 | RUST | [x] | [x] | — | — | [x] | [x] | `virtual_disk` 5개 테스트, 전체 대상 59개 통과; `cargo check --locked` 통과; VDI 파서·UI는 후속 ID 범위 |
| D-001 | RUST | [x] | [x] | — | — | [x] | [x] | `config::tests` 6개 통과(구버전 기본값·새 작업 기본값·직렬화 왕복 포함); `cargo check -p gpui-convenience-tools --locked` 통과; 설정 입력 UI는 D-004 범위 |
| D-002 | RUST | [x] | [x] | — | — | [x] | [x] | `sync::tests` glob 전용 3개 통과; `*`·`?`는 단일 경로 조각, 독립 `**`는 0개 이상 하위 폴더, `/`·`\\` 구분자 정규화 확인; `cargo test --all-targets --all-features --locked` 통과 |
| D-003 | RUST | [x] | [x] | — | — | [x] | [x] | `excluded_files_are_not_copied_and_count_as_skipped`, `excluded_directories_are_not_created_or_removed_by_mirror_deletes` 통과; 상대 경로 glob 적용, `skipped` 계상, 제외 디렉터리의 하위 순회·미러 삭제 보호 확인; 전체 테스트 통과 |
| D-005 | RUST | [x] | [x] | — | — | [x] | [x] | 제외 파일 2개가 대상에 복사되지 않고 `skipped=2`가 되는 회귀 테스트와 제외 디렉터리 보호 테스트 통과; `cargo test --all-targets --all-features --locked` 통과 |
| D-006 | RUST | [x] | [x] | — | — | [x] | [x] | 3,000개 파일 3회 측정에서 사전 순회 `0.003초`, 실제 동기화 `3.262~3.308초`, 추가 비용 약 `0.1%`; 종료 TACS `TACS.vdi` NTFS 파티션 1의 읽기 전용 트리 1회 `0.524초`·2회 `0.127초`; 사전 순회 채택 결정과 수동 측정 테스트 고정; `git diff --check` 통과 |
| D-007 | RUST | [x] | [x] | — | — | [x] | [x] | `count_sync_entries`·`SyncProgress.total`·백그라운드 이벤트 전달 구현; `pre_scan_counts_the_same_processable_entries_as_sync_progress` 통과; `cargo check --locked`, 전체 173 passed·5 ignored, Clippy `-D warnings`, 문서·구조 게이트 통과; commit `4e29479` push 완료 |
| D-008 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `file_sync_status_bar_stays_visible_at_compact_height_while_running`에서 920×480 bounds와 진행률 바 포함을 확인; 전체 173 passed·5 ignored, 필수 GPUI 31개, Clippy `-D warnings`; release 실제 동기화 중 캡처 `target/visual-validation/captures/d008-progress-running-1000x700-205653.png`에서 `동기화 중`, `복사 6135 · 건너뜀 4000 · 실패 0`과 진행률 바 확인; 세션 종료 후 process/session 0, commit `4e29479` push 완료 |
| D-004 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `file_sync_exclude_patterns_editor_is_multiline_and_contained`로 920/994/1280px 입력 영역의 멀티라인 높이·설정 카드 내부 경계 확인; `file_sync_run_button_saves_current_inputs_and_queues_selected_job`로 공백·빈 줄 정리와 줄바꿈 입력의 저장·실행 연결 확인; 격리 릴리즈 캡처는 `target/visual-validation/captures/d4-exclude-patterns-994-editor-visible-170031.png`, `d4-exclude-patterns-1280-editor-visible-170018.png`; 커밋·푸시 완료 |
| D-020 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 최근 이력 상태·건수·소요 시간 단위 테스트와 920×480·994×702·1280×900 카드 경계 GPUI 테스트 통과. 최소 높이에서 페이지 overflow·wheel offset·마지막 `sync-history-row-2`의 뷰포트 진입 단언을 추가해 독립 검토의 최초 `FAIL` 사유를 해소. 전체 186 passed·5 ignored, 필수 GPUI 34개, Clippy `-D warnings`. 독립 Visual Reviewer가 제품 commit `478a4c4`의 release SHA-256 `21D17114A4FBC90C6D5AF9B6979A91F976B69182CDD550E88A589A2C49FE6866`을 별도 격리 실행하고 자체 캡처 `d020-independent-first-920-160505.png`, `d020-independent-920-compact-bottom-161016.png`, `d020-independent-994-compact-bottom-161030.png`, `d020-independent-1280-compact-bottom-161030.png`에서 스크롤바 드래그로 마지막 이력·건수·소요 시간·경계를 확인한 뒤 보강 테스트를 독립 재실행해 최종 `PASS` 판정. 실제 첫 wheel 입력 2회는 화면을 이동시키지 않아 성공으로 주장하지 않으며, 검증 PID·격리 루트 제거를 확인. |
| D-014 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 기본 Skip, 원본 내부 Follow, 대상 내부 상대 링크 Recreate 정책·경고를 확정. 숨김·시스템 항목 포함 직접 선택과 링크 모드 GPUI 테스트, 원본 밖·순환·대상 링크 탈출 테스트 통과. 격리 release SHA-256 `4EEA9866630D195F149314AF466CD08C475D6457CDE125545264DA55CF26D2B9` 캡처 `d014-file-sync-full-2200-150006.png`(1280px)·`d014-file-sync-920-full-150327.png`에서 숨김/시스템 폴더·제외 버튼·링크 모드·카드 경계 확인; 검증 PID/세션 루트 0, 임시 대상 미생성. 구현 commit `c94b74f` push 완료. |
| D-015 | RUST | [x] | [x] | — | — | [x] | [x] | Windows 파일/폴더 심볼릭 링크와 정션의 Follow·Recreate 구현, 동일 링크 재실행 건너뜀 및 다른 링크·일반 파일 충돌 비덮어쓰기 테스트. 전체 186 passed·5 ignored, 관리자 권한 링크 테스트 exit 0, Clippy `-D warnings` 통과. 구현 commit `c94b74f` push 완료. |
| D-016 | RUST | [x] | [x] | — | — | [x] | [x] | 링크 생성 오류 1314를 관리자 권한/개발자 모드 안내가 있는 경로별 실패로 변환. 관리자 권한 테스트 바이너리 `sync::link_tests` exit 0. 권한 없는 1314 실기계 조건은 재현되지 않아 오류 생성 단위 테스트로 메시지를 검증. 구현 commit `c94b74f` push 완료. |
| VDE-020 | DOCS | — | — | — | — | [x] | [x] | `PROJECT_MAP.md`의 50개 Rust 파일·19,486줄·모듈별 책임/줄 수와 `MASTER_PLAN.md` Phase O-18 이력을 실측 갱신; `git diff --check` 및 활성 ID 정합성 assertion 통과; Phase O 전체 완료 표시는 VDE-019 후속; commit `840a35e` push 완료 |
| VDE-013 | GPUI | [x] | [x] | [x] | [x] | [x] | [x] | `virtual_disk_panel_registers_navigation_and_renders_read_only_shell`; release `CLAUDE_LOCAL` 캡처 `vde013-panel-920-final-201106.png`, `vde013-panel-1280-final-201106.png`; 후속 실제 VirtualBox 7.2.14 VDI 캡처 `vde019-vbox-standard-loaded-023912.png`에서 VDI 경로·MBR/NTFS 파티션·게스트 루트 연결을 확인하고 새로고침 경계를 GPUI 테스트로 검증; 세션 종료 후 프로세스·세션 루트 0; commit `83c579b` |
| T-001 | RUST | [x] | [x] | — | — | [x] | [x] | `overwrites_an_existing_readonly_target_file`로 읽기 전용 대상 파일 속성 해제·덮어쓰기·최종 내용 확인; `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (139 passed, 4 ignored); Clippy exit 0, 기존 경고 7건; commit `8d14458` push 완료 |
| T-002 | RUST | [x] | [x] | — | — | [x] | [x] | `update_config_preserves_unedited_fields`로 서비스·타겟·즐겨찾기·동기화 진행 정보·로그 설정 보존과 디스크 재로드 확인; 임시 `GPUI_CONVENIENCE_TOOLS_DATA_DIR` 격리; `cargo check --locked`; 전체 139 passed·4 ignored; Clippy 기존 경고 7건; commit `8d14458` push 완료 |
| G-002 | RUST | [x] | [x] | — | — | [x] | [x] | `AppRoot` 단일 GPUI 엔티티를 유지하면서 `ServiceState`·`SyncState`·`AdBlockState`로 기능별 상태 소유권 분리; `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` 전체 139 passed·4 ignored; Clippy 기존 경고 7건; 실제 UI 동작 변경이 없는 구조 리팩터링으로 E/V는 N/A; commit `3185369` push 완료 |
| VDE-004 | RUST | [x] | [x] | — | — | [x] | [x] | `virtual_disk::vdi` 4개 테스트 통과; VDI 1.1 헤더·블록 맵·동적 미할당/고정 이미지 읽기·차등 이미지 거부·중복/범위/오버플로 검증; `cargo test --all-targets --all-features --locked`, `cargo check -p gpui-convenience-tools --locked`, `git diff --check` 통과 |
| VDE-005 | RUST | [x] | [x] | — | — | [x] | [x] | `virtual_disk::vdi` 9개 테스트 통과; `.lck` 표식 거부·환경변수 기반 VBoxManage 경로 대조 함수·read-only 핸들·크기/mtime 변경 감지; `cargo test -p gpui-convenience-tools virtual_disk::vdi --locked`, 전체 `cargo test --all-targets --all-features --locked`, `cargo check -p gpui-convenience-tools --locked`, `git diff --check` 통과 |
| VDE-006 | RUST | [x] | [x] | — | — | [x] | [x] | `virtual_disk::partition` 6개 테스트 통과; MBR 기본·확장 EBR 논리 파티션·protective MBR/GPT·주·백업 GPT 헤더 CRC·MBR 범위 초과 검증; `cargo test -p gpui-convenience-tools --all-targets --all-features --locked`, `cargo check -p gpui-convenience-tools --locked`, `cargo build -p gpui-convenience-tools --locked`, `git diff --check` 통과 |
| VDE-007 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools`; `cargo test -p gpui-convenience-tools` (95 passed, 3 ignored); env 주입 실제 NTFS 이미지 테스트 1 passed; commit·push 완료 |
| VDE-008 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (99 passed, 4 ignored); NTFS 오류 경계 7 passed; 실제 NTFS 이미지 1 passed; 손상 부트 섹터 이미지 1 passed; commit·push 완료 |
| VDE-009 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test -p gpui-convenience-tools virtual_disk::copy --all-targets --all-features` (6 passed); `cargo test --all-targets --all-features` (105 passed, 4 ignored); 경로 정책 오류 리뷰 완료; commit·push 완료 |
| VDE-010 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test -p gpui-convenience-tools virtual_disk::copy --all-targets --all-features` (7 passed) 및 `virtual_disk::path_policy` (6 passed); `cargo test --all-targets --all-features` (112 passed, 4 ignored); 청크/충돌 오류 리뷰 완료; commit·push 완료 |
| VDE-011 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 메타데이터 적용·실패 기록 4개 및 NTFS 시간 변환 1개 단위 테스트; `cargo test --all-targets --all-features` (117 passed, 4 ignored); commit·push 완료 |
| VDE-018 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test -p gpui-convenience-tools virtual_disk:: --all-targets --all-features` (56 passed, 2 ignored); `cargo test --all-targets --all-features` (135 passed, 4 ignored); `cargo clippy --all-targets --all-features` exit 0 (기존 경고 7건); `app/testdata/ntfs-testfs1.img` 기반 합성 MBR+동적 VDI에서 NTFS 루트·파일 읽기와 원본 바이트 불변성 검증, 고정 이미지 read-only 및 복사본 손상 테스트; rustfmt 대상 파일·`git diff --check`; commit·push 완료 |
| D-017 | RUST | [x] | [x] | — | — | [x] | [x] | `skips_symbolic_links_without_reporting_a_sync_failure`가 실제 Windows 심볼릭 링크를 복사하지 않고 `skipped=1`·실패 목록 비어 있음·일반 파일 복사 성공을 확인; `cargo check -p gpui-convenience-tools --locked`; 전체 141 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `6cf305c` push 완료 |
| D-018 | RUST | [x] | [x] | — | — | [x] | [x] | `sync_history::tests` 2개로 실행 순서 append·손상 JSON 보존을 검증; 동기화 백그라운드 완료 경로에서 작업 ID·라벨·시각·복사/건너뜀/삭제/실패·중지·요약을 `sync-history.json`에 기록; `cargo check -p gpui-convenience-tools --locked`; 전체 143 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `97009c3` push 완료 |
| D-019 | RUST | [x] | [x] | — | — | [x] | [x] | `sync_history::tests::applies_log_count_and_age_retention_before_appending`로 로그 설정의 개수·기간 기준을 새 이력 추가 전에 적용하고 최소 1개를 유지하는 경계 검증; `cargo check -p gpui-convenience-tools --locked`; 전체 144 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `ed71af4` push 완료 |
| D-009 | RUST | [x] | [x] | — | — | [x] | [x] | workspace와 `app`에 `notify 7` 직접 의존성을 선언하고 `Cargo.lock` 애플리케이션 의존성 목록을 갱신; `cargo check -p gpui-convenience-tools --locked`; `cargo test -p gpui-convenience-tools --all-targets --all-features` 전체 145 passed·4 ignored; `cargo tree -e normal -p gpui-convenience-tools`에서 앱 직접 `notify v7.0.0` 확인; Clippy exit 0(기존 경고 7건); `git diff --check` 통과; 실시간 감시 동작은 D-010~D-013 범위로 유지; commit `0315bbd` push 완료 |
| D-010 | RUST | [x] | [x] | — | — | [x] | [x] | `SyncJob`에 `WatchMode::{Interval, Realtime}`와 snake_case 직렬화를 추가하고 `watch_mode` 누락 구버전 설정을 `Interval`로 복원; `sync_job_watch_mode_round_trips_with_interval_compatibility`, `sync_job_without_id_gets_one`; `cargo check -p gpui-convenience-tools --locked`; 전체 146 passed·4 ignored; Clippy exit 0(기존 경고 7건); E/V는 스키마 전용 작업으로 적용하지 않음; 실제 감시·디바운스·설정 UI는 D-011~D-013에서 검증; commit `15e9765` push 완료 |
| D-011 | RUST | [x] | [x] | — | — | [x] | [x] | `app/src/app/watch.rs`에 `notify` 재귀 감시·작업별 2초 quiet debounce·변경 이벤트 기반 실행 예약을 구현; `debounce_coalesces_changes_until_two_seconds_are_quiet`, `realtime_watcher_reports_a_changed_file_after_polling`; 실제 임시 폴더 파일 변경 통합 테스트 포함; `cargo check -p gpui-convenience-tools --locked`; 전체 151 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `e68a3f6` push 완료 |
| D-012 | RUST | [x] | [x] | — | — | [x] | [x] | watcher 생성·경로 감시 실패를 작업별 한 번만 보고하고 `SyncWatchFallback` 이벤트로 주기 모드 강등·로그·경고 토스트를 처리; `watch_failure_is_reported_once_until_the_job_mode_changes`, `file_sync_watch_fallback_downgrades_and_logs`; 전체 151 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `e68a3f6` push 완료 |
| D-013 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 파일 동기화 설정에 `실시간 감시` 토글을 추가하고 실시간 모드에서는 주기 선택을 숨김; `file_sync_watch_mode_toggle_updates_selected_job`, `Verify-Workspace.ps1` 필수 GPUI 20개; 릴리스 `CLAUDE_LOCAL` 캡처 `target/visual-validation/captures/d011-realtime-panel-015924.png`에서 작업 카드의 `실시간` 상태·토글·2초 안내 확인; 종료 후 processCount=0·sessionCount=0; commit `e68a3f6` push 완료 |

### 합성 VDI 릴리스 E2E

실제 사용자 VDI와 사용자 데이터는 검증에 사용하지 않는다. 먼저 환경 변수로 합성 VDI를
export하고, 하네스가 세션 전용 `source\seed.vdi`와 `target`을 만든 뒤 실제 release 앱에
주입한다.

```powershell
$env:GPUI_CONVENIENCE_TOOLS_EXPORT_VDI_FIXTURE = "D:\검증\vde019-synthetic-ntfs.vdi"
cargo test -p gpui-convenience-tools virtual_disk::vdi::tests::exports_ntfs_vdi_fixture_when_requested -- --nocapture

scripts\Invoke-ClaudeVisualCheck.ps1 -Action Start `
  -SeedVdi target\visual-validation\vde019-synthetic-ntfs.vdi `
  -AutoCopyVdi -InitialPanel VirtualDisk -Width 1200 -Height 1000
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name vde019-copy-summary
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
```

`-AutoCopyVdi`는 검증 전용 환경 변수로만 선택 항목을 임시 대상에 복사한다. 캡처 후에는
`processCount=0`, `sessionCount=0`, 임시 대상 루트 삭제를 확인한다. 이 경로는 합성 VDI와
실제 release 앱의 통합 증거이며, VirtualBox가 직접 생성한 사용자 VDI와 독립 Visual Reviewer
검토를 대신하지 않는다.

대용량 종료 VM 디스크를 검증할 때는 원본을 세션 폴더로 복사하지 않도록
`-ExternalVdiPath <absolute-vdi-path>`를 사용한다. 앱은 이 경로를 `VdiReader`의 read-only
입력으로만 열고, 대상 폴더·설정·로그는 세션 임시 루트에 격리한다. 하네스는 설치된
`VBoxManage.exe`를 환경 변수로 주입해 실행 중 VM 가드를 활성화한다. 이 옵션은 원본 파일을
복사하지 않으므로 실제 사용자 VDI에 사용할 때도 VM이 종료된 상태인지 먼저 확인해야 한다.

2026-09-22 현재 종료된 `TACS` VM의 실제 VDI를 이 경로로 열어 `TACS.vdi`에서 GPT 파티션
1·5의 NTFS 3.1과 게스트 루트의 시스템 폴더를 확인했다. 920×700 캡처
`target/visual-validation/captures/vde019-external-tacs-root-fixed-920x700-053642.png`와
1200×1000 캡처 `target/visual-validation/captures/vde019-external-tacs-root-fixed-1200x1000-053659.png`를
남겼으며, 원본 길이는 변하지 않았고 세션 종료 후 `PROCESS_COUNT=0`·`SESSION_COUNT=0`이었다.
대용량 파일 중지·외부 키보드·독립 Visual Reviewer는 이 검증으로 완료 처리하지 않는다.

복사 중지 경계는 검증 전용 `-CancelAfterMs <milliseconds>`로도 재현할 수 있다. 이 옵션은
`-AutoCopyVdi`와 함께 사용할 때 복사 시작 후 앱의 중지 메서드를 호출하며, 설정 파일이나
일반 사용자 실행 경로에는 저장하지 않는다. 종료된 실제 `TACS.vdi`의 200ms 시나리오에서
release 앱 요약 `중지됨 · 파일 0개 · 0 B · 건너뜀 0 · 실패 0`을 확인했고 캡처
`target/visual-validation/captures/vde015-external-tacs-cancel-1200x1000-054536.png`를
남겼다. 같은 VDI의 1,500ms 실행은 실제 루트 데이터가 먼저 끝나 `완료 · 파일 19개 · 4.1 MB`가
되었고, `$Extend`의 1,000ms 실행도 `완료 · 파일 8개 · 4.1 MB`가 되어 실제 대용량 파일의
청크 중간 중지는 재현되지 않았다. 따라서 이 시드는 중지 상태·정리 경계의 보조 증거일 뿐,
VDE-015의 실제 대용량 파일 중지 완료 증거로 승격하지 않는다.

다중 파티션 VDI를 검증할 때는 `-VdiPartitionNumber <번호>`를 지정한다. 지정하지 않으면
기존 호환 동작대로 첫 번째 파티션을 선택한다. 실제 종료된 TACS VM에서 파티션 5번을
선택한 캡처 `target/visual-validation/captures/vde015-tacs-partition5-root-060904.png`를
확인했고, 파티션 1번 `$Extend` 경로의 `-CancelAfterMs 250`은
`중지됨 · 파일 0개 · 0 B`로 끝났으며 캡처는
`target/visual-validation/captures/vde015-tacs-extend-cancel-061128.png`이다. 같은 경로의
`600`ms 실행은 `완료 · 파일 8개 · 4.1 MB`였고 캡처는
`target/visual-validation/captures/vde015-tacs-extend-cancel-600-061151.png`이다. 두 실행은
원본 VDI를 변경하지 않았지만 실제 대용량 파일의 청크 중간 중지를 증명하지 않으므로 VDE-015
완료 조건으로 승격하지 않는다.

같은 종료된 TACS VDI의 `$Extend/$RmMetadata/$TxfLog`에는 64.0KB 파일과 2MiB 파일 2개가
있어, 자연 속도에서 먼저 완료되는 작은 항목과 큰 항목의 중지 경계를 분리해 확인했다.
검증 전용 `-ReadDelayMs 1000 -CancelAfterMs 2200`을 사용했으며 이 지연은 일반 사용자
설정에 저장되지 않고 복사 작업 스레드의 게스트 `read_at` 호출에만 적용된다. 실제 release
앱 캡처 `target/visual-validation/captures/vde015-txf-log-slow-chunk-before-cancel-062617.png`
와 `target/visual-validation/captures/vde015-txf-log-slow-chunk-after-cancel-062619.png`에서
동일한 4개 선택 항목과 `중지됨 · 파일 2개 · 64.0 KB` 요약을 확인했다. 실행 중 대상에는
`$TxfLogContainer00000000000000000001` 0바이트 파일이 생성된 상태가 관찰되었고, 취소 후
그 미완성 파일은 제거되며 이미 완료된 `$TxfLog.blf`만 남았다. 원본 길이
`85,269,151,744`바이트 불변, VM `poweroff`, 세션 종료 후 `PROCESS_COUNT=0`·
`SESSION_COUNT=0`을 확인했다. 이는 실제 VDI의 파일·복사 엔진·부분 파일 정리 경계를
잇는 보조 E2E 증거이며, 자연 속도의 청크 중지나 독립 Visual Reviewer 확인을 완료 처리하지
않는다.

2026-09-22 2차 자체 시각 교차 확인으로 D-020·G-001의 최신 release 화면을 별도 격리
세션에서 다시 열었다. D-020은 994×702에서 최근 실행 이력 카드가 세로 스크롤 안에
배치되고, 1280×900에서 중지·실패 포함·성공 3행이 모두 카드 경계 안에 표시되었다.
캡처는 `target/visual-validation/captures/d020-second-pass-before-055457.png`와
`target/visual-validation/captures/d020-second-pass-resized-055501.png`이다. G-001은
994×702·1280×900 모두 자동 시작 상태 카드·작업 제어·안내 문장이 폭 밖으로 밀리지 않았다.
캡처는 `target/visual-validation/captures/g001-second-pass-before-055522.png`와
`target/visual-validation/captures/g001-second-pass-resized-055525.png`이다. 이는 같은
에이전트의 별도 세션에서 수행한 2차 자체 확인이며 독립 Visual Reviewer 검토를 대체하지
않는다. 각 세션 종료 후 프로세스와 세션 루트는 0개였다.

폴더 진입 화면을 입력 없이 재현할 때는 `-InitialGuestPath many_subdirs`를 함께 사용한다.
이 값은 검증 프로세스에만 전달되며 제품 설정에는 저장되지 않는다.

### 증거 기록 형식

증거 칸에는 긴 로그를 복사하지 말고 재현 가능한 식별자만 적는다.

```text
명령: cargo test -p gpui-convenience-tools <test_name> -- --nocapture
결과: PASS
수용 기준: <무엇을 확인했는가>
관찰: <실제 결과>
E2E/시각 증거: <PNG 경로 또는 N/A 사유>
오류 리뷰: <없음 또는 오류 ID/원인>
커밋: <commit hash>, origin/main 푸시 확인
```

작업을 완료 처리하면서 행을 삭제하지 않는다. 완료 행은 `MASTER_PLAN.md`의 완료 기록과
커밋을 연결하는 검증 이력으로 남긴다. 단, `TODO.md`에서 완료 작업을 삭제하는 기존 규칙은
그대로 유지한다.

## 진행 중 항목의 기존 검증 근거

아래 결과는 기존 기록의 보존이며 미완료 게이트를 새로 통과시킨 것이 아니다.

<a id="c002-settings-evidence"></a>

### C-002 — 기존 검증과 미검증 경계

현재 AppData 구형 JSON 끝의 중괄호 초과를 읽기 전용으로 확인. 격리 복구·원본 불변·전체 설정/색상 모드 왕복·손상 설정 보존·동시 변경·새 설정 우선순위 및 GPUI Light/Dark 버튼 입력 테스트 추가. `config/persistence.rs`는 동시 갱신을 직렬화하고 임시 파일 완성 후 교체; 설정 읽기/저장 실패는 앱 로그에 표시. 전체 199 passed·7 ignored, `cargo check --locked`, Clippy `-D warnings`, 문서·구조 검사 통과. 사용자 승인 후 구 릴리즈를 정상 종료하고 release SHA-256 `67D9F1F6F219F2327427D3447A98B8E9EAE7D11B09B66202257480987667F80F` 빌드. `scripts/Test-SettingsPersistence.ps1`가 격리 프로세스 30956·31028을 차례로 시작/정상 종료해 구형 손상 파일 불변, 새 설정 파일 생성, 광고 자리 회수=true·색상 모드=dark·스캔 37초·서비스/동기화 off·즐겨찾기 유지 확인(`SETTINGS_E2E_PASSED`). 1000×700 실제 릴리즈 설정 화면 `target/visual-validation/captures/c002-dark-settings-restart-150840.png`에서 어두운 모드·카드 경계 확인; 검증 세션과 프로세스 정리. 2026-09-30 표시 검증은 기존 증거로 충족. 남은 일: 기존 사용자 프로필에서 광고 자리 회수를 다시 선택한 뒤 재시작 유지 여부 1회 확인. 격리 검증을 사용자 프로필 확인으로 표시하지 않음.

<a id="g003-splitter-evidence"></a>

### G-003 — 기존 검증과 미검증 경계

`sidebar_width` 설정 필드와 200~360px 보정·기본 240px 복원; `sidebar_width_is_normalized_to_supported_range`, `update_config_preserves_unedited_fields`, GPUI `sidebar_divider_drag_resizes_navigation_and_content`; 전체 최신 테스트는 상단 품질 게이트 기준 161 passed·4 ignored; 격리 release 기본 캡처 `target/visual-validation/captures/g003-default-003709.png`, `sidebar_width=320` 시드 복원 캡처 `target/visual-validation/captures/g003-restored-width-003742.png`; 최신 release 시작 캡처 `target/visual-validation/captures/next-g003-before-065645.png`에서 divider 위치를 재확인했으나 `-Action Drag -X 0.25 -Y 0.50 -ToX 0.32 -ToY 0.50`은 포그라운드 안전 검사(`현재 포그라운드=66246`)에서 차단되어 입력을 보내지 않음; 2026-09-30 표시 검증은 기존 증거로 충족. 남은 일: 저장을 활성화한 GPUI drag/mouse-up→설정 재로드→새 화면 폭 복원 단언. 외부 입력 반복·독립 재조작은 선택
