# gpui-convenience-tools — Master Plan

> **정체성**: Rust + GPUI 기반 **다용도 데스크탑 보조 도구 모음**.
> 개별 기능이 아니라 *편의 기능을 담는 그릇*이 이 프로젝트의 정체성이다.
> 새 기능은 기존 기능과 결합하지 않는 **독립 패널**로 추가한다.
>
> **기술 스택**: Rust 2021 · GPUI 0.2.2 · gpui-component 0.5.1 · windows-sys 0.52 · tokio
> **참고**: [longbridge/gpui-component](https://github.com/longbridge/gpui-component)

---

## 프로젝트 구조

```text
gpui-convenience-tools/          ← 저장소 루트
├── Cargo.toml                   # workspace (members = ["app"])
├── MasterPlan.md / TODO.md / PROJECTMAP.md / README.md / CLAUDE.md / AGENTS.md
├── app/                         ← 유일한 크레이트 (package name = gpui-convenience-tools)
│   ├── Cargo.toml
│   ├── build.rs                 # /MANIFEST:NO (gpui 임베드 매니페스트 중복 방지)
│   ├── assets/themes/           # 번들 테마 JSON 21종
│   └── src/
│       ├── main.rs              # 진입점: 로거 설치 → 테마 시드 → 윈도우 오픈
│       ├── config.rs            # AppConfig · SyncJob · LogConfig · update_config
│       ├── logging.rs           # 롤링 파일 로거 (log::Log 구현)
│       ├── sync.rs              # 폴더 동기화 엔진
│       ├── app/                 # AppRoot(Render) · 상태 · 이벤트 루프 · 사이드바
│       │   ├── mod.rs           #   구조체 · 생성자 · 최상위 레이아웃
│       │   ├── state.rs         #   순수 데이터 타입
│       │   ├── background.rs    #   스캔 · 동기화 백그라운드 스레드
│       │   ├── ops.rs           #   광고 차단 · 서비스 · 로그 설정 조작
│       │   ├── sync_ops.rs      #   파일 동기화 작업 조작
│       │   ├── events.rs        #   백그라운드 → UI 이벤트 처리
│       │   └── inputs.rs        #   입력 위젯 지연 생성
│       ├── platform/
│       │   ├── mod.rs           # Platform trait (창 조작 + SCM)
│       │   └── windows/         # Win32 구현
│       │       ├── mod.rs       #   WindowsPlatform + Platform impl
│       │       ├── window_ops.rs#   창 · 프로세스 열거
│       │       ├── tray.rs      #   시스템 트레이
│       │       ├── scm.rs       #   Windows 서비스 등록 · 서비스 모드
│       │       ├── services.rs  #   설치된 서비스 조회 · 제어
│       │       └── task_scheduler.rs # 로그온 자동 시작
│       └── window/
│           ├── mod.rs           # 패널 모듈 + scroll_pane 헬퍼
│           ├── ad_block.rs      # 편의 기능: 웹뷰 광고 차단
│           ├── file_sync.rs     # 편의 기능: 파일 동기화
│           ├── service_mgr.rs   # 편의 기능: Windows 서비스
│           ├── service_view.rs  # 시스템: 자동 시작(작업 스케줄러)
│           └── settings.rs      # 시스템: 전역 설정(테마 · 로그 보관)
└── installer/windows/
```

---

## 아키텍처 원칙

### 1. 편의 기능 = 독립 패널 + 스플리터

각 편의 기능 페이지는 `h_resizable`로 좌우가 나뉜다.

| 영역 | 내용 |
| --- | --- |
| 왼쪽 (기능 영역) | 목록, 상태, 실행 결과 |
| 오른쪽 (설정 영역) | **그 기능에만 해당하는** 설정 |

전역 설정 페이지(`settings.rs`)에는 앱 전체에 걸친 것(테마, 로그 보관)만 둔다.
기능별 설정을 전역 설정에 섞지 않는 것이 이 구조의 핵심이다.

### 2. 상태 흐름

```text
       UI (AppRoot)
         │  ▲
 공유    │  │ PlatformEvent 채널
 뮤텍스  ▼  │ (백그라운드 → UI 단방향)
   ScannerState        SyncSharedState
         │                   │
   광고 스캔 스레드      동기화 스레드
   (tokio current_thread)   (std::thread, 1초 틱)
```

- **UI → 백그라운드**: 공유 뮤텍스에 쓰고 동기화 함수 호출
- **백그라운드 → UI**: `PlatformEvent` 채널만 사용. UI 핸들러도 채널을 경유한다
- 파일 I/O는 블로킹이므로 동기화 스레드를 광고 스캔 스레드와 분리했다

### 3. 설정 저장 단일 경로

`config::update_config(|cfg| ...)` 하나만 사용한다. 읽기-수정-쓰기를 한 곳에 모아
필드가 늘어날 때 저장 지점마다 복사 로직을 빠뜨려 값이 유실되는 문제를 막는다.

### 4. 구조 리팩터링 트리거 (1,000줄)

줄 수는 원인이 아니라 **증상**이다. 1,000줄 초과는 책임 배치가 프로젝트 규모를 못 따라왔다는
신호이므로, 그 시점에 하는 일은 파일 자르기가 아니라 **기능 관점의 구조 재설계**다.
트리거된 파일만 보지 않고 프로젝트 전체에서 같은 책임이 흩어진 곳을 함께 정리한다.

순서: **① 중복 제거(공용 유틸 승격) → ② 오배치 책임 이동 → ③ 책임 단위 분할.**
①~②에서 임계값 아래로 내려가면 ③은 하지 않는다 — 파일 개수를 늘리는 것이 목적이 아니다.

### 5. 공용 유틸 단일 소유 (상시 적용)

같은 일을 하는 헬퍼는 이름이 달라도 중복으로 보고 한 곳으로 합친다.
GPUI 엘리먼트를 반환하는 UI 프리미티브는 `window/ui.rs`, 순수 함수는 주인이 있는 도메인
모듈(`config`·`sync`·`logging`), Win32 래퍼는 `platform/windows/mod.rs`가 소유한다.
패널 파일에는 그 기능 고유의 헬퍼만 남긴다.

리팩터링은 동작 변경 없이 수행하고 결과를 `PROJECTMAP.md`에 기록한다.
규칙 정본은 `.github/copilot-instructions.md`의 「구조 리팩터링 기준」과
「공용 유틸 승격 기준」 절이다.

---

## 구현 완료 단계

### Phase 1 — 프로젝트 초기화 ✅

cargo workspace, Hello World 앱 동작 확인

### Phase 2 — Platform 추상화 ✅

`Platform` trait(창 조작), `WindowsPlatform` Win32 구현

### Phase 3 — GPUI UI 기반 ✅

사이드바 네비게이션, 패널 전환, 테마 21종, 가상 리스트 로그, 커스텀 타이틀바

### Phase 4 — Windows 트레이 & 자동 시작 ✅

시스템 트레이 최소화, 작업 스케줄러 기반 자동 시작(ONLOGON, Session 0 격리 우회)

### Phase A — 프로젝트 리네임 ✅

`webview-ad-ban-gpui` → `gpui-convenience-tools`

### Phase B — Windows 서비스 관리 ✅

`EnumServicesStatusEx` 기반 목록, 시작/중지/삭제, 관리자 권한 확인

### Phase C — 정체성 재정립 & 편의 기능 확장 ✅

- 사이드바를 **개요 / 편의 기능 / 시스템** 3그룹으로 재편, 기능 명칭 정리
- 모든 편의 기능 페이지에 스플리터(기능 영역 / 설정 영역) 적용
- **파일 동기화** 기능 신규 구현(엔진 + 패널 + 실패 알림 억제)
- **롤링 파일 로거** 구현(개수 · 날짜 · 용량 3중 보존 기준)
- 사용하지 않던 `window/dashboard.rs`, `target_list.rs`, `log_view.rs` 제거
- 문서 전면 개편, `copilot-instructions.md` 인코딩 손상 복구

### Phase G — 1,000줄 규칙 도입과 구조 분할 ✅

- 지침에 「파일 크기 기준(1,000줄 규칙)」과 「프로젝트 맵 관리 기준」 추가
- `PROJECTMAP.md` 신규 — 파일 구조·줄 수·책임 추적
- `app.rs`(1,798줄) → `app/` 7파일 분할, 대시보드·로그 렌더는 `window/`로 이동
- `platform/windows.rs`(1,361줄) → `platform/windows/` 6파일 분할
- 결과: 최대 파일 690줄, 1,000줄 초과 파일 없음

### Phase H — 규칙을 구조 리팩터링 관점으로 승격 ✅

기존 규칙이 "파일 자르기"로 읽혀 줄 수 지표만 내려가고 중복·오배치는 남는 문제가 있었다.

- 「파일 크기 기준」 → **「구조 리팩터링 기준(1,000줄 트리거)」**: 기계적 분할 금지,
  ① 중복 제거 → ② 오배치 책임 이동 → ③ 책임 단위 분할 순서로 재정의
- **「공용 유틸 승격 기준」 신설** — 1,000줄과 무관하게 상시 적용, 「즉시」 판정은 즉시 처리
- `PROJECTMAP.md`에 「공용 유틸 인벤토리」·「중복 헬퍼 추적」 추가

### Phase I — 공용 UI 프리미티브 승격 ✅

- `window/ui.rs` 신설 — `badge` · `action_button` + `Tone` / `ButtonStyle` / `Size`
- 패널마다 흩어져 있던 정의 4개(`ad_block::badge`, `service_view::state_badge`,
  `file_sync::action_button`, `service_view::action_button`)와 인라인 8곳을 흡수
- 패널 4개 합계 140줄 감소, 색 선택은 `ui.rs`가 `cx.theme()`에서 직접 읽도록 일원화
- `service_view` 버튼이 쓰던 `sidebar_primary` → `primary` 교정(사이드바 토큰 오용)
- 문서 정리: 완료 이력은 이 문서로, 미착수는 `TODO.md`로 모으고 `TASKS.md` 제거

---

## 검증 재개 단계

### Phase J — 테마 가시성·스크롤 안정화 🧪

구현 변경은 반영되었지만 새 GPUI 자체 테스트 필수 게이트를 소급 적용해 최종 완료 판정을
재개했다. `TODO.md`의 네이티브 테스트와 실제 화면 순차 검증을 모두 통과하기 전에는 완료로
분류하지 않는다.

- 사이드바 그룹과 개별 항목에 `sidebar_border` 기반 경계를 적용해 비활성 항목 구분 강화
- 번들 테마 36개 변형의 스위치 팔레트를 감사하고, 누락 토큰을 런타임 최소 대비 정책으로 보정
- `window::ui::toggle_switch`로 스위치 외곽선 정책을 공용화하고 직접 `Switch::new` 사용 제거
- 파일 동기화 좌·우 컨텐츠의 고정 높이(`size_full`/세로 `flex_1`)를 제거해 자동 스크롤 복구
- 광고 차단·파일 동기화·서비스 관리의 고정 초기 폭을 공용 `balanced_split`으로 교체해
  기능·설정 pane이 가용 너비를 함께 사용하고 설정 영역 최소 300px을 유지하도록 개선
- 사이드바 전체를 독립 `scroll_pane`으로 감싸 작은 창에서도 마지막 시스템 항목까지 도달
- GPUI 자체 테스트로 기본 1000×700·최소 폭 920px의 pane 너비와 920×480의 사이드바
  wheel 스크롤·마지막 항목 도달을 검증
- Codex·Claude 공용 정본과 GPUI 스킬에 테마·스크롤 회귀 방지 규칙 반영
- Computer Use health check와 구현 담당자·독립 Visual Reviewer의 순차 크로스체크 계약 및
  Codex·Claude Code 에이전트 연결 추가
- `TestAppContext`/`VisualTestContext` 기반 수용 기준별 GPUI 회귀 테스트를 모든 UI 변경의
  필수 완료 조건으로 정본·스킬·Codex/Claude 브리지에 반영

---

## 진행 예정 단계

세부 체크리스트는 `TODO.md`를 정본으로 한다.

### Phase D — 파일 동기화 고도화 🗓

- 실시간 감시(`notify` 크레이트, 이미 의존성 트리에 존재)
- 제외 패턴(glob) 지원
- 진행률 표시 및 취소
- 심볼릭 링크/정션 처리 옵션

### Phase E — 편의 기능 확장 🗓

후보: 클립보드 히스토리, 스크린샷 도구, 프로세스 모니터, 빠른 실행기

### Phase F — macOS 지원 🗓 (미정)

- `platform/macos.rs` 구현(현재 파일 없음, `Platform` trait 기본 구현으로 컴파일만 통과)
- launchd 기반 자동 시작

---

## 빠른 명령

```powershell
cargo run   -p gpui-convenience-tools
cargo check -p gpui-convenience-tools
cargo test  -p gpui-convenience-tools -- --nocapture
cargo build -p gpui-convenience-tools --release
```

---

## 참고 링크

- gpui-component: [longbridge/gpui-component](https://github.com/longbridge/gpui-component)
- GPUI: [gpui.rs](https://gpui.rs)
- Component Gallery: [gpui-component gallery](https://longbridge.github.io/gpui-component/gallery/)
- windows-sys: [docs.rs/windows-sys](https://docs.rs/windows-sys)
