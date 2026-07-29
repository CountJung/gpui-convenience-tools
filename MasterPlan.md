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
├── MasterPlan.md / TASKS.md / TODO.md / README.md / CLAUDE.md / AGENTS.md
├── app/                         ← 유일한 크레이트 (package name = gpui-convenience-tools)
│   ├── Cargo.toml
│   ├── build.rs                 # /MANIFEST:NO (gpui 임베드 매니페스트 중복 방지)
│   ├── assets/themes/           # 번들 테마 JSON 21종
│   └── src/
│       ├── main.rs              # 진입점: 로거 설치 → 테마 시드 → 윈도우 오픈
│       ├── app.rs               # AppRoot(Render) · 상태 · 이벤트 루프 · 사이드바
│       ├── config.rs            # AppConfig · SyncJob · LogConfig · update_config
│       ├── logging.rs           # 롤링 파일 로거 (log::Log 구현)
│       ├── sync.rs              # 폴더 동기화 엔진
│       ├── platform/
│       │   ├── mod.rs           # Platform trait (창 조작 + SCM)
│       │   └── windows.rs       # Win32 구현 · 트레이 · 작업 스케줄러 · SCM
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
