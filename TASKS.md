# gpui-convenience-tools — 작업 현황

## 완료

- [x] 테마 선택 저장/복원 (`config.json` light/dark theme name)
- [x] 설정 화면 테마 검색/필터 UI 추가
- [x] gpui-component 번들 테마 확장 (21개)
- [x] 콘텐츠 영역 자동 세로 스크롤바 적용
- [x] Windows 시스템 트레이 최소화 기능
- [x] 작업 스케줄러 기반 자동 시작 (ServiceView 패널)
- [x] Virtual list 기반 로그 패널
- [x] ServiceView 카드 앱 영역 오버플로우 수정
  - `app.rs` 콘텐츠 영역: `size_full()` → `flex_1().min_w_0().h_full()`
  - `service_view.rs`: 루트에 `min_w_0()`, 각 카드에 `w_full()` 추가

---

## 진행 중

### Phase A — 프로젝트 리네임

- [x] `adblocker/Cargo.toml` → `name = "gpui-convenience-tools"`
- [x] `Cargo.toml` workspace 패키지명/경로 확인
- [x] `app.rs` 창 제목 문자열 변경
- [x] `app.rs` 알림 타이틀 변경 (Notification title)
- [x] `service_view.rs` task name 문자열 변경
- [x] `config.rs` 설정 파일 경로 변경 (`%APPDATA%/gpui-convenience-tools/`)
- [x] `installer/windows/*.wxs` 파일 업데이트 (파일명 포함)
- [x] `MasterPlan.md` / `TASKS.md` / `AGENTS.md` / `copilot-instructions.md` 정리
- [x] `.vscode/tasks.json` / `launch.json` 업데이트
- [ ] 기존 Git 저장소 제거 후 새 저장소(`gpui-convenience-tools`) 재생성

---

## 예정

### Phase B — Windows 서비스 관리 패널

> 상세 계획: MasterPlan.md Phase B 참조

#### B-1. Platform 레이어 확장 (platform/windows.rs)
- [ ] `ServiceInfo`, `ServiceStatus`, `ServiceStartType` 구조체 정의
- [ ] `Platform` trait에 서비스 관련 메서드 추가
  - `list_services() -> Result<Vec<ServiceInfo>>`
  - `start_service(name: &str) -> Result<()>`
  - `stop_service(name: &str) -> Result<()>`
  - `query_service(name: &str) -> Result<ServiceInfo>`
- [ ] `Cargo.toml` windows-sys features에 `Win32_System_Services`, `Win32_Security` 추가
- [ ] Win32 구현: `OpenSCManager`, `EnumServicesStatusEx`, `StartService`, `ControlService`
- [ ] 관리자 권한 확인 (`IsUserAnAdmin`) 및 elevated 재시작 유도

#### B-2. 서비스 관리 패널 UI (window/service_mgr.rs)
- [ ] `window/service_mgr.rs` 신규 파일 생성
- [ ] 헤더 (제목 + 새로고침 버튼)
- [ ] 검색 입력창 (InputState 사용)
- [ ] 서비스 목록 Virtual List
  - 상태 배지 (Running=success, Stopped=muted, Pending=warning)
  - 시작 / 중지 / 재시작 버튼 (권한 없으면 비활성)
- [ ] 관리자 권한 안내 배너

#### B-3. AppRoot 연동 (app.rs)
- [ ] `ActivePanel::ServiceMgr` variant 추가
- [ ] 사이드바 "Services" 네비게이션 항목 추가
- [ ] `render_service_mgr_panel()` 연동
- [ ] 서비스 제어 이벤트 처리 (`PlatformEvent` 확장)

#### B-4. 설정 연동
- [ ] `AppConfig`에 `favorite_services: Vec<String>` 추가
- [ ] 즐겨찾기 서비스 필터 UI (설정 패널 또는 서비스 관리 패널 내)

#### B-5. 검증
- [ ] `cargo check` 통과
- [ ] `cargo build` 통과
- [ ] 실제 서비스 시작/중지 동작 확인 (관리자 권한 환경)

---

### Phase C — macOS 지원 확장 (미정)
- [ ] macOS launchd 서비스 관리 stub 설계

---

## 빠른 명령

```bash
# 실행 (리네임 후)
cargo run -p gpui-convenience-tools

# 점검
cargo check -p gpui-convenience-tools

# 테스트
cargo test -p gpui-convenience-tools -- --nocapture

# 릴리즈
cargo build -p gpui-convenience-tools --release
```

