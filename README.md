# gpui-convenience-tools

Rust + GPUI 기반 데스크톱 광고 차단 도구입니다.

## 주요 기능

- 카카오톡 프로세스(WebView 윈도우) 감지 및 광고 창 숨김
- GPUI UI 기반 대시보드, 타겟 앱 토글, 로그 뷰
- 앱 활성화/비활성화 전역 스위치
- 백그라운드 플랫폼 스캐너(`tokio::spawn`) + UI 이벤트 채널 연동
- 설정 파일 영속화
  - Windows: `%APPDATA%/gpui-convenience-tools/config.json`
- gpui-component 테마 JSON 번들 자동 로드
  - `%APPDATA%/gpui-convenience-tools/themes/*.json`로 시드
- 선택한 라이트/다크 테마 이름 저장 및 재시작 복원

## 요구 사항

- Rust stable
- Windows (`windows-sys` 기반 구현)

## 실행

```bash
cargo run -p gpui-convenience-tools
```

## 검증

```bash
cargo check -p gpui-convenience-tools
cargo test -p gpui-convenience-tools
```

## 릴리즈 빌드

```bash
cargo build -p gpui-convenience-tools --release
```

릴리즈 빌드에는 UAC 매니페스트(`requireAdministrator`)가 임베드되어  
실행 시 Windows 사용자 계정 컨트롤(UAC) 승격 프롬프트가 표시됩니다.  
광고 창 조작을 위한 `EnumWindows` / `ShowWindow` 호출에 관리자 권한이 필요합니다.

## 자동 시작 (Task Scheduler)

### 개요

Windows 서비스(SCM)는 Session 0(비대화형 세션)에서 실행되기 때문에  
사용자 세션의 창(KakaoTalk 등)을 `EnumWindows` / `ShowWindow`로  
조작할 수 없습니다(**Session 0 격리 문제**).  
이 앱은 이 문제를 회피하기 위해 **Windows 작업 스케줄러**를 사용합니다.

### 동작 흐름

```
부팅
  └→ 사용자 로그온
       └→ Task Scheduler: ONLOGON 트리거 (+ /IT 인터랙티브 플래그)
            └→ gpui-convenience-tools.exe --tray
                 ├→ 앱 정상 기동 (스캔 루프 시작)
                 ├→ 300ms 후 시스템 트레이로 자동 숨김
                 └→ EnumWindows / ShowWindow 정상 동작 (Session 1, 사용자 세션)
```

### 작업 스케줄러 등록 방법

앱 내 **Service** 탭에서 UI로 등록/삭제/즉시 실행이 가능합니다.  
CLI로 직접 관리할 경우:

```powershell
# 등록
schtasks /Create /TN "gpui-convenience-tools" /TR '\"<설치경로>\gpui-convenience-tools.exe\" --tray' /SC ONLOGON /IT /F

# 삭제
schtasks /Delete /TN "gpui-convenience-tools" /F

# 즉시 실행
schtasks /Run /TN "gpui-convenience-tools"
```

### --tray 플래그

작업 스케줄러가 앱을 실행할 때 `--tray` 플래그를 전달합니다.  
이 플래그가 있으면 앱은 UI 창 없이 트레이 아이콘으로 최소화 상태로 시작되며,  
백그라운드 스캔 루프만 동작합니다.

## 관리자 권한 디버깅 (VS Code)

디버그 빌드(`cargo build` 기본)에는 UAC 매니페스트가 없습니다.  
VS Code에서 관리자 권한으로 디버깅하는 방법은 두 가지입니다.

### 방법 1 — VS Code를 관리자로 실행

1. VS Code를 우클릭 → **관리자 권한으로 실행**
2. launch.json의 `Debug gpui-convenience-tools (Admin)` 구성 사용

VS Code 프로세스가 관리자이면, 자식 프로세스(디버그 exe)도 관리자 권한으로 실행됩니다.

### 방법 2 — Attach 디버깅

1. 관리자 권한 터미널에서 `target\debug\gpui-convenience-tools.exe` 직접 실행
2. VS Code에서 launch.json의 `Attach to gpui-convenience-tools` 구성 사용
3. 프로세스 목록에서 `gpui-convenience-tools` 선택하여 attach

## Windows 설치 파일(MSI) 생성

WiX Toolset(v3) 설치 후 아래를 실행합니다.

```powershell
.\installer\windows\build-installer.ps1
```

생성 결과:

- `installer/windows/gpui-convenience-tools.msi`

## CI / CD

### PR & push 검증 (`windows-build.yml`)

`main` / `master` 브랜치 push 또는 PR 시 자동 실행됩니다.

- `cargo check -p gpui-convenience-tools`
- `cargo test -p gpui-convenience-tools`
- `cargo build -p gpui-convenience-tools --release`

### 릴리즈 자동 게시 (`release.yml`)

`v*` 태그를 push하면 GitHub Release가 자동 생성됩니다.

```bash
git tag v1.0.0
git push origin v1.0.0
```

- Release 빌드 후 `gpui-convenience-tools.exe`를 릴리즈 에셋으로 첨부합니다.
- 릴리즈 노트는 커밋 기록을 기반으로 자동 생성됩니다.

## 프로젝트 문서

- 계획: `MasterPlan.md`
- 작업 상태: `TASKS.md`
- 코파일럿 메인 지침: `.github/copilot-instructions.md`



