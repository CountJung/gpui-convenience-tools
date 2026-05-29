# gpui-convenience-tools — Agent Master Plan

> **목적**: Rust + GPUI + gpui-component 기반의 크로스플랫폼 편의 도구 모음.  
> 초기 기능은 Windows WebView2 광고 차단(KakaoTalk 등)이며, Windows / macOS 환경에서  
> 유용한 시스템 유틸리티를 점진적으로 추가하는 것을 목표로 한다.
>
> **기술 스택**: Rust 2021 · GPUI 0.2.2 · gpui-component 0.5.1 · windows-sys 0.52 · tokio  
> **참고**: [longbridge/gpui-component](https://github.com/longbridge/gpui-component)

---

## 프로젝트 구조

```text
gpui-convenience-tools/        ← 저장소 루트
├── Cargo.toml                 # workspace root
├── Cargo.lock
├── MasterPlan.md
├── TASKS.md
├── AGENTS.md
├── adblocker/                 ← 메인 크레이트
│   ├── Cargo.toml             # name = "gpui-convenience-tools"
│   ├── build.rs               # Windows .ico / manifest 임베드
│   ├── assets/
│   │   └── themes/            # 21개 JSON 테마
│   └── src/
│       ├── main.rs
│       ├── app.rs             # AppRoot (Render), AppState, 전체 레이아웃
│       ├── config.rs
│       ├── window/
│       │   ├── mod.rs
│       │   ├── dashboard.rs
│       │   ├── target_list.rs
│       │   ├── settings.rs
│       │   ├── log_view.rs
│       │   └── service_view.rs  # 자동 시작 관리 (Windows)
│       └── platform/
│           ├── mod.rs
│           ├── windows.rs
│           └── macos.rs
└── installer/
    └── windows/
```

---

## 구현 완료 단계

### Phase 1 — 프로젝트 초기화 ✅
- cargo workspace 생성, Hello World 앱 동작 확인

### Phase 2 — Platform 추상화 ✅
- `Platform` trait (is_target_running, find_ad_window, hide_ad, show_ad)
- `WindowsPlatform` Win32 구현 + macOS stub

### Phase 3 — GPUI UI 구현 ✅
- Sidebar 네비게이션 + 패널 전환 구조
- Dashboard / TargetList / Settings / Logs / ServiceView 패널
- 21개 테마 번들 + 검색/저장/복원
- Virtual list 로그 패널 + 커스텀 타이틀바

### Phase 4 — Windows 트레이 & 자동 시작 ✅
- 시스템 트레이 최소화
- 작업 스케줄러 기반 자동 시작 (ONLOGON, Session 1 격리 우회)
- ServiceView 패널 (등록 / 삭제 / 지금 실행 / 상태 표시)

---

## 진행 예정 단계

### Phase A — 프로젝트 리네임 🔄
**목표**: `webview-ad-ban-gpui` → `gpui-convenience-tools`

#### 체크리스트
- [ ] Cargo.toml (workspace 및 adblocker) package name 변경
- [ ] app.rs 창 제목·알림 타이틀 문자열 변경
- [ ] service_view.rs task name 문자열 변경
- [ ] config.rs 설정 경로 변경
- [ ] installer WXS 파일 업데이트
- [ ] MasterPlan.md / TASKS.md / AGENTS.md 정리
- [ ] 기존 Git 저장소 제거 후 새 저장소 재생성

---

### Phase B — Windows 서비스 관리 패널 🗓
**목표**: Windows 서비스(SCM)를 GUI로 관리하는 새 패널 추가  
**플랫폼**: Windows 전용 (`#[cfg(target_os = "windows")]`)  
**사이드바**: "Services" 항목

#### 기능 범위

| 기능 | 설명 |
| --- | --- |
| 서비스 목록 조회 | `EnumServicesStatusEx` 로 실행 중·중단 서비스 목록 |
| 서비스 시작 | `StartService` |
| 서비스 중지 | `ControlService(SERVICE_CONTROL_STOP)` |
| 서비스 재시작 | 중지 후 시작 순차 실행 |
| 상태 실시간 갱신 | 주기적 `QueryServiceStatusEx` 폴링 |
| 즐겨찾기 필터 | 자주 쓰는 서비스만 표시 (config.json 저장) |
| 이름 검색 | 서비스 이름/표시 이름으로 필터링 |

#### 아키텍처

```text
window/
  service_mgr.rs          ← 새 파일 (서비스 관리 패널 UI)

platform/windows.rs       ← ServiceManager 확장
  list_services()         → Vec<ServiceInfo>
  start_service(name)     → Result<()>
  stop_service(name)      → Result<()>
  query_service(name)     → Result<ServiceStatus>

app.rs
  ActivePanel::ServiceMgr ← 새 패널 variant
  render_service_mgr_panel()
```

#### 데이터 구조

```rust
#[derive(Clone, Debug)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub status: ServiceStatus,
    pub start_type: ServiceStartType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Paused,
    StartPending,
    StopPending,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServiceStartType {
    Automatic,
    Manual,
    Disabled,
}
```

#### Win32 API 목록

| API | 용도 |
| --- | --- |
| `OpenSCManager` | SCM 핸들 획득 |
| `EnumServicesStatusEx` | 서비스 목록 조회 |
| `OpenService` | 개별 서비스 핸들 |
| `StartService` | 서비스 시작 |
| `ControlService(SERVICE_CONTROL_STOP)` | 서비스 중지 |
| `QueryServiceStatusEx` | 상태 조회 |
| `CloseServiceHandle` | 핸들 해제 |

#### UI 구성 (service_mgr.rs)

```text
v_flex (패널 루트)
├── 헤더: "Windows 서비스 관리" + [새로고침] 버튼
├── 검색 입력창
├── 서비스 목록 (Virtual List)
│   └── 행: [상태 배지] [표시 이름] [내부 이름] [시작/중지/재시작]
└── 관리자 권한 안내 배너 (UAC 필요 시)
```

#### 권한 처리 전략
- 서비스 시작/중지는 관리자 권한 필요
- non-elevated 실행 중이면 `ShellExecuteExW(runas)` 로 elevated 재시작 유도
- 또는 elevated 서브프로세스(`--service-cmd start <name>`) spawn

#### 예상 작업 기간: 1~2주

---

### Phase C — macOS 지원 확장 🗓 (미정)
- macOS launchd 기반 서비스 관리 (launchctl)

---

## 빠른 명령

```bash
# 실행
cargo run -p gpui-convenience-tools

# 점검
cargo check -p gpui-convenience-tools

# 릴리즈
cargo build -p gpui-convenience-tools --release
```

---

## 참고 링크

- gpui-component: [longbridge/gpui-component](https://github.com/longbridge/gpui-component)
- GPUI: [gpui.rs](https://gpui.rs)
- Component Gallery: [gpui-component gallery](https://longbridge.github.io/gpui-component/gallery/)
- windows-sys: [docs.rs/windows-sys](https://docs.rs/windows-sys)

