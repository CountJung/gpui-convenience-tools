# gpui-convenience-tools — 작업 현황

> 단계 계획은 `MasterPlan.md`, 미착수 구현 대기열은 `TODO.md`를 참조한다.
> 이 문서는 **완료된 작업**과 **현재 진행 중인 작업**만 기록한다.

---

## 완료

### Phase C — 정체성 재정립 & 편의 기능 확장

#### C-1. 사이드바 · 레이아웃 재편

- [x] 사이드바를 **개요 / 편의 기능 / 시스템** 3그룹으로 분리 (`NAV_TOOLS`, `NAV_SYSTEM`)
- [x] 기능 명칭 정리: `Targets` → **웹뷰 광고 차단**, `Services` → **Windows 서비스**,
      `Service` → **자동 시작**, 신규 **파일 동기화**
- [x] 각 항목에 한 줄 설명 추가(기능을 모르는 사용자도 이해 가능하게)
- [x] 편의 기능 페이지에 스플리터(`h_resizable`) 적용 — 좌: 기능 영역 / 우: 설정 영역
  - [x] `ad_block.rs` — 상태·타겟 목록 / 스캔 주기·프로세스 추가
  - [x] `file_sync.rs` — 작업 목록·실패 기록 / 작업 설정
  - [x] `service_mgr.rs` — 서비스 목록 / 검색·상태 필터·권한 상태
- [x] `window::scroll_pane` 헬퍼 추가(스플리터 각 영역의 독립 스크롤)
- [x] 기능별 설정을 전역 설정 페이지에서 분리(스캔 주기 → 광고 차단 페이지로 이동)
- [x] 사용하지 않던 `window/dashboard.rs`, `window/target_list.rs`, `window/log_view.rs` 제거
      (렌더는 `app.rs` 인라인이었고 이 파일들은 `#[allow(dead_code)]` 잔재였음)

#### C-2. 파일 동기화 (신규)

- [x] `config::SyncJob` 정의(원본·대상·주기·자동 여부·숨김 포함·미러 삭제)
- [x] `sync.rs` 동기화 엔진
  - [x] 재귀 순회 + 크기/수정시각 기반 복사 판정(2초 여유로 FAT 정밀도 대응)
  - [x] 숨김·시스템 속성 파일 포함(기본값)
  - [x] 읽기 전용 대상 파일 속성 해제 후 덮어쓰기
  - [x] 미러 삭제(원본에 없는 대상 항목 제거)
  - [x] 대상이 원본 내부인 경우 무한 복사 차단
  - [x] Windows 오류 코드 → 한국어 사유 변환(5/32/33/112/206/1920)
- [x] 전용 동기화 스레드(`spawn_sync_loop`, 1초 틱) — 파일 I/O 블로킹을 광고 스캔과 분리
- [x] 작업 식별을 인덱스 → **고유 ID**로 변경
      (인덱스 방식은 실행 중 작업 삭제 시 실행 주기·결과가 다른 작업에 잘못 붙는 문제가 있었음.
      구버전 config는 `SyncJob::ensure_id()`로 로드 시 백필)
- [x] `window/file_sync.rs` 패널 — 작업 목록 · 상태 · 실패 기록 / 작업 설정
- [x] 네이티브 폴더 선택 대화상자(`prompt_for_paths` + `spawn_in`)
- [x] 실패 알림 억제
  - [x] 이미 목록에 있는 실패는 토스트 재표시 안 함
  - [x] 항목별 `알림 끄기` 토글
  - [x] `실패 알림 표시` 전체 스위치
- [x] 단위 테스트 5종(중첩 복사·재복사·미러 삭제·내부 대상 차단·원본 없음)

#### C-3. 롤링 파일 로그

- [x] `logging.rs` — `log::Log` 구현(콘솔 + 파일 동시 기록)
- [x] 3중 보존 기준: 파일 개수 / 보관 기간(일) / 파일당 용량(MB)
- [x] 용량 초과 시 `app-YYYYMMDD-HHMMSS.log`로 롤링(동일 초 충돌 시 접미사)
- [x] 런타임 설정 변경 즉시 반영(`logging::update_config`)
- [x] Windows `GetLocalTime` 기반 지역 시각(비Windows는 UTC 폴백)
- [x] `env_logger` 의존성 제거 — `RUST_LOG`는 콘솔 레벨 제어로 유지
- [x] 파일 기록 기본 상한 `Debug` — gpui의 프레임 단위 vsync TRACE 로그가 파일을 채워
      실제 앱 로그가 롤링으로 밀려나는 문제 해결(20초 idle 기준 1047B → 321B)
- [x] 설정 페이지에 로그 보관 설정 UI 추가
- [x] 로그 패널 헤더에 파일 개수·총 용량·경로 표시
- [x] UI 로그 엔트리 상한 2000개(메모리 무한 증가 방지)

#### C-4. 구조 정리

- [x] `config::update_config` 단일 저장 경로 도입 — 저장 지점 3곳의 수동 필드 복사 제거
      (필드 추가 시 값 유실되던 문제 해소)
- [x] `AppConfig`에 `sync_jobs`, `log` 추가(모두 `#[serde(default)]`)
- [x] `favorite_services` 실제 사용 — 서비스 목록 ★ 토글 + 즐겨찾기 필터
- [x] `Cargo.toml`에 `Win32_System_SystemInformation` 피처 추가

#### C-5. 문서 개편

- [x] `README.md` 전면 개편 — 정체성/기능 표/화면 구성/동기화·로그 상세
- [x] `MasterPlan.md` 재작성 — 아키텍처 원칙(패널 구조·상태 흐름·저장 경로) 명문화
- [x] `TASKS.md` 재작성
- [x] `TODO.md` 신규 — 단계적 구현 대기열
- [x] `CLAUDE.md` 갱신
- [x] `.github/copilot-instructions.md` **인코딩 손상 복구** + 정체성·패널 구조 기준 추가
- [x] `.github/instructions/kakao-gpui-core.instructions.md` →
      `gpui-core.instructions.md` 리네임, `applyTo` 경로 수정(`adblocker/` → `app/`)
- [x] `.github/agents/error-reviewer.agent.md` `argument-hint` 인코딩 손상 복구
- [x] `AGENTS.md` 참조 지도 갱신

#### C-6. 검증

- [x] `cargo check` 통과 (경고 0)
- [x] `cargo test` 통과 (12 passed / 1 ignored)
- [x] `cargo build` (debug) 통과
- [x] `cargo check --release` 통과
- [x] 디버그 빌드 실행 스모크 테스트 — 창 생성, 로거 설치, 로그 파일 기록 확인
- [x] 구버전 config.json 하위 호환 확인 (신규 필드 없이도 정상 로드)
- [ ] `cargo build --release` **링크** — 실행 중인 트레이 인스턴스가 exe를 잠가 `os error 5`.
      앱 종료 후 재시도 필요(코드 문제 아님, `cargo check --release`는 통과)
- [ ] 실제 폴더 동기화 동작 확인(대용량·사용 중 파일·숨김/시스템 속성)
- [ ] 로그 롤링 실제 동작 확인(용량 초과 유도)

---

### Phase G — 1,000줄 규칙 도입과 구조 분할

#### G-1. 지침 · 추적 문서

- [x] `.github/copilot-instructions.md`에 「파일 크기 기준(1,000줄 규칙)」 추가
      — 임계값 표(800/1,000), 분할 방법 6단계, 예외 조건
- [x] 「프로젝트 맵 관리 기준」 추가 — 갱신 시점과 줄 수 실측 명령
- [x] `PROJECTMAP.md` 신규 — 27개 파일의 줄 수·책임·분할 이력·다음 분할 후보
- [x] `gpui-core.instructions.md` `applyTo`에 `PROJECTMAP.md` 추가
- [x] `AGENTS.md` / `CLAUDE.md` / `MasterPlan.md` / `README.md` 참조 갱신

#### G-2. `app.rs` 분할 (1,798줄 → 7파일, 최대 564줄)

- [x] `app/mod.rs` — 구조체·생성자·사이드바·최상위 레이아웃
- [x] `app/state.rs` — 순수 데이터 타입
- [x] `app/background.rs` — 스캔·동기화 스레드
- [x] `app/ops.rs` — 광고 차단·서비스·로그 설정 조작
- [x] `app/sync_ops.rs` — 파일 동기화 작업 조작
- [x] `app/events.rs` — 이벤트 소비·토스트 유틸
- [x] `app/inputs.rs` — 입력 위젯 지연 생성
- [x] 대시보드·로그 패널 렌더를 `window/dashboard.rs`, `window/log_view.rs`로 이동
      (패널 렌더는 `window/`에 둔다는 기존 규약에 맞춤)

#### G-3. `platform/windows.rs` 분할 (1,361줄 → 6파일, 최대 344줄)

- [x] `windows/mod.rs` — `WindowsPlatform` + `Platform` 구현, re-export
- [x] `windows/window_ops.rs` — 창·프로세스 열거
- [x] `windows/tray.rs` — 시스템 트레이
- [x] `windows/scm.rs` — Windows 서비스 등록·서비스 모드 실행
- [x] `windows/services.rs` — 설치된 서비스 조회·제어
- [x] `windows/task_scheduler.rs` — 로그온 자동 시작
- [x] 공용 헬퍼 `wide_null`을 `windows/mod.rs`로 승격(tray·services 공용)

#### G-4. 검증

- [x] `cargo check` 통과 (경고 0)
- [x] `cargo test` 통과 (12 passed / 1 ignored) — 분할 전과 동일
- [x] `cargo build` (debug) 통과
- [x] 실행 스모크 테스트 — 창 생성·로그 파일 기록 동일 확인
- [x] 동작 변경 없음(순수 이동). 로직 수정은 포함하지 않음

---

### Phase H — 1,000줄 규칙을 구조 리팩터링 규칙으로 승격 (문서)

기존 규칙이 "파일 자르기"로 읽혀 줄 수 지표만 내려가고 중복·오배치는 남는 문제가 있었다.
**줄 수는 증상이고 조치는 기능 관점의 구조 재설계**임을 규칙에 명시했다. 코드 변경 없음.

- [x] `.github/copilot-instructions.md` — 「파일 크기 기준」을 **「구조 리팩터링 기준(1,000줄 트리거)」**로 개편
      — 기계적 분할 금지 명시, 절차를 **① 중복 제거 → ② 오배치 책임 이동 → ③ 책임 단위 분할** 순서로 재정의
      (①~②에서 임계값 아래로 내려가면 ③ 생략)
- [x] `.github/copilot-instructions.md` — **「공용 유틸 승격 기준」 신설**(1,000줄 트리거와 무관하게 상시 적용)
      — 중복 판정(이름이 달라도 같은 일이면 중복, 인라인 반복도 중복), 2곳=후보 / 3곳=즉시 승격,
      승격 위치 표, `cx.theme()` 직접 읽기 원칙, 원본 삭제 의무
- [x] 「프로젝트 맵 관리 기준」 확장 — 공용 유틸 인벤토리·중복 헬퍼 추적 유지 의무 추가
- [x] `PROJECTMAP.md` — 제목·성격을 **구조·크기·공용 유틸 추적**으로 개편,
      **「공용 유틸 인벤토리」·「중복 헬퍼 추적」 신설**(실측 6건), 「다음 분할 후보」 → 「다음 리팩터링 후보」
      (1순위=중복 제거, 2순위=재배치·분할), 「분할 이력」 → 「리팩터링 이력」(종류 열 추가)
- [x] `CLAUDE.md` · `MasterPlan.md` · `AGENTS.md` · `README.md` · `gpui-core.instructions.md` 참조 갱신
- [x] 중복 실측 — 상태 배지 3곳, 액션 버튼 3곳+, 스탯 타일 2곳, 토글 행 2곳+, 간격 표기 4곳
      → 실행 항목은 `TODO.md`「3. 구조 개선」의 `window/ui.rs` 신설로 등록

---

## 이전 완료 (요약)

- Phase 1~4: workspace 초기화, `Platform` 추상화, GPUI UI, 트레이 + 작업 스케줄러 자동 시작
- Phase A: `webview-ad-ban-gpui` → `gpui-convenience-tools` 리네임
- Phase B: Windows 서비스 관리(목록·시작·중지·삭제·권한 확인)

---

## 진행 중

없음. 다음 작업은 `TODO.md`에서 선택한다.

---

## 빠른 명령

```powershell
cargo run   -p gpui-convenience-tools
cargo check -p gpui-convenience-tools
cargo test  -p gpui-convenience-tools -- --nocapture
cargo build -p gpui-convenience-tools --release
```
