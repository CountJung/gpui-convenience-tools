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
