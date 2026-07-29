# gpui-convenience-tools

**Rust + GPUI 기반 다용도 데스크탑 보조 도구 모음.**

하나의 앱 안에 서로 독립적인 편의 기능을 패널 단위로 모아 두는 것이 이 프로젝트의 목적이다.
개별 기능이 아니라 **편의 기능을 담는 그릇**이 정체성이므로, 새 기능은 기존 기능과 얽히지 않는
독립 패널로 추가된다.

## 편의 기능

| 기능 | 설명 | 플랫폼 |
| --- | --- | --- |
| **웹뷰 광고 차단** | 카카오톡 등 WebView2 기반 앱의 광고 창을 감지해 숨긴다. | Windows |
| **파일 동기화** | 원본 폴더를 대상 폴더로 주기적으로 복사한다. 숨김·시스템 파일 포함. | 공통 |
| **Windows 서비스** | 설치된 Win32 서비스를 조회·시작·중지·삭제한다. | Windows |

부가 요소로 대시보드, 자동 시작 등록(작업 스케줄러), 롤링 파일 로그, 21종 테마를 제공한다.

### 화면 구성

좌측 사이드바는 **개요 / 편의 기능 / 시스템** 세 그룹으로 나뉜다.
각 편의 기능 페이지는 스플리터로 좌우가 나뉘며, 드래그로 폭을 조절할 수 있다.

```text
┌──────────────┬──────────────────────────────┬───────────────────┐
│ 사이드바      │ 기능 영역                     │ 설정 영역          │
│              │ (목록 · 상태 · 실행 결과)      │ (그 기능의 설정)   │
│ 개요         │                              │                   │
│  대시보드     │  ←──────── 스플리터로 폭 조절 ────────→          │
│ 편의 기능     │                              │                   │
│  웹뷰 광고 차단│                              │                   │
│  파일 동기화  │                              │                   │
│  Windows 서비스│                             │                   │
│ 시스템        │                              │                   │
│  자동 시작    │                              │                   │
│  로그         │                              │                   │
│  설정         │                              │                   │
└──────────────┴──────────────────────────────┴───────────────────┘
```

앱 전역 설정(테마, 로그 보관)만 **설정** 페이지에 있고, 기능별 설정은 각 기능 페이지 우측에 있다.

## 요구 사항

- Rust stable
- Windows 10/11 (광고 차단·서비스 관리는 `windows-sys` 기반이라 Windows 전용)

## 실행과 검증

```powershell
cargo run   -p gpui-convenience-tools            # 실행
cargo check -p gpui-convenience-tools            # 코드 수정 후
cargo test  -p gpui-convenience-tools -- --nocapture
cargo build -p gpui-convenience-tools --release  # 릴리즈
```

실행 플래그: `--tray`(창 없이 트레이로 시작), `--service`(SCM 서비스 디스패처 모드).

## 저장 위치

모든 사용자 데이터는 `%APPDATA%\gpui-convenience-tools\` 아래에 있다.

```text
%APPDATA%\gpui-convenience-tools\
├── config.json     # 설정(타겟 앱, 동기화 작업, 테마, 로그 설정)
├── themes\         # 번들 테마 JSON 21종 (최초 실행 시 시드, 이후 감시)
└── logs\
    ├── app.log                     # 현재 로그
    └── app-20260729-142530.log     # 롤링된 로그
```

## 파일 동기화

원본 폴더의 파일을 대상 폴더로 주기적으로 복사한다. **숨김·시스템 속성 파일을 포함해
전체를 동기화**하는 것이 기본 동작이다.

- 복사 판정: 크기가 다르거나 원본이 2초 이상 최신이면 복사(내용 해시 비교는 하지 않음)
- 감시 주기: 작업별로 30초 ~ 1시간 중 선택
- 옵션: 자동 동기화 사용 / 숨김·시스템 파일 포함 / 원본에서 삭제된 항목 반영(미러 삭제)

### 동기화 실패 처리

복사할 수 없는 파일(사용 중·권한 부족·경로 길이 초과 등)은 **건너뛰고 사유를 남긴다**.
사유는 세 곳에 남는다.

1. **토스트 알림** — 새로 발생한 실패에 대해서만 표시
2. **로그 패널 + 롤링 로그 파일** — 항상 기록
3. **파일 동기화 패널의 실패 목록** — 경로별 사유

같은 파일에서 같은 사유가 반복될 때 알림이 계속 뜨는 것을 막기 위해 두 가지 억제 수단이 있다.

- 실패 항목별 **`알림 끄기`** 버튼 — 해당 항목의 토스트만 끈다(로그에는 계속 기록)
- **`실패 알림 표시`** 스위치 — 동기화 실패 토스트 전체를 끈다

이미 목록에 있는 실패는 다시 토스트를 띄우지 않으므로, 매 주기마다 같은 알림이 반복되지 않는다.

## 로그

콘솔과 파일에 동시에 기록하며, 파일은 세 가지 보존 기준을 함께 적용한다.
**설정 → 로그 파일 보관**에서 변경하며, 변경 즉시 반영된다.

| 기준 | 설정값 | 동작 |
| --- | --- | --- |
| 파일 개수 | 3 / 5 / 10 / 20 / 50개 | 현재 파일 포함 개수 초과 시 오래된 순 삭제 |
| 보관 기간 | 제한 없음 / 7 / 14 / 30 / 90일 | 기준보다 오래된 파일 삭제 |
| 파일 용량 | 1 / 5 / 10 / 50 / 100 MB | 초과 시 타임스탬프 이름으로 롤링 |

콘솔 출력 레벨은 `RUST_LOG` 환경변수로 조정한다(기본 `info`).
파일에는 `Debug` 레벨까지 기록된다 — GPUI가 프레임마다 남기는 `Trace` 로그가 파일을 가득 채워
실제 앱 로그를 밀어내기 때문이다. `RUST_LOG=trace`를 지정하면 파일 상한도 함께 올라간다.

## 관리자 권한

Windows 서비스 시작·중지·삭제에는 관리자 권한이 필요하다.
앱은 실행 시 권한을 확인해 **Windows 서비스** 페이지 우측에 현재 권한 상태를 표시한다.

> **참고**: 디버그·릴리즈 빌드 모두 UAC 승격 매니페스트가 **임베드되지 않는다.**
> `app/resources.rc`와 `app/gpui-convenience-tools.exe.manifest`에 `requireAdministrator` 설정이
> 남아 있지만, `build.rs`가 이를 컴파일하지 않는다(아래 [매니페스트 제약](#매니페스트-제약) 참조).
> 관리자 권한이 필요하면 앱을 **관리자 권한으로 직접 실행**해야 한다.

### 매니페스트 제약

`gpui`의 `windows-manifest` 피처가 이미 `RT_MANIFEST(ID=1)`을 임베드하기 때문에,
`winres` / `embed-resource`로 매니페스트를 추가하면 `CVT1100` / `LNK1123` 중복 리소스 오류가 난다.
`gpui`에 `default-features = false`를 지정해도 `gpui-component`가 기본 피처로 다시 켜므로 효과가 없다.
그래서 `build.rs`는 `/MANIFEST:NO`만 지정하고 자체 매니페스트 임베딩을 하지 않는다.

## 자동 시작 (작업 스케줄러)

Windows 서비스(SCM)는 Session 0(비대화형)에서 실행되어 사용자 세션의 창을
`EnumWindows` / `ShowWindow`로 조작할 수 없다(**Session 0 격리**).
이 문제를 피하기 위해 자동 시작은 **작업 스케줄러**를 사용한다.

```text
부팅 → 사용자 로그온
       └→ Task Scheduler: ONLOGON 트리거 (+ /IT 인터랙티브 플래그)
            └→ gpui-convenience-tools.exe --tray
                 ├→ 스캔·동기화 루프 시작
                 ├→ 300ms 후 트레이로 자동 숨김
                 └→ EnumWindows / ShowWindow 정상 동작 (사용자 세션)
```

앱 내 **자동 시작** 페이지에서 등록/삭제/즉시 실행이 가능하다. CLI로 직접 관리하려면:

```powershell
schtasks /Create /TN "gpui-convenience-tools" /TR '\"<설치경로>\gpui-convenience-tools.exe\" --tray' /SC ONLOGON /IT /F
schtasks /Delete /TN "gpui-convenience-tools" /F
schtasks /Run    /TN "gpui-convenience-tools"
```

## 관리자 권한 디버깅 (VS Code)

- **방법 1** — VS Code를 관리자 권한으로 실행한 뒤 `Debug gpui-convenience-tools (Admin)` 구성 사용.
  부모 프로세스가 관리자이면 자식(디버그 exe)도 관리자 권한으로 실행된다.
- **방법 2** — 관리자 터미널에서 `target\debug\gpui-convenience-tools.exe`를 직접 실행하고,
  `Attach to gpui-convenience-tools` 구성으로 attach.

## Windows 설치 파일(MSI)

WiX Toolset(v3) 설치 후:

```powershell
.\installer\windows\build-installer.ps1
```

결과: `installer/windows/gpui-convenience-tools.msi`

## CI / CD

- **`windows-build.yml`** — `main`/`master` push 및 PR에서 `cargo check` → `cargo test` → 릴리즈 빌드
- **`release.yml`** — `v*` 태그 push 시 릴리즈 빌드 + MSI를 GitHub Release 에셋으로 업로드

```bash
git tag v1.0.0
git push origin v1.0.0
```

## 프로젝트 문서

| 문서 | 내용 |
| --- | --- |
| `MasterPlan.md` | 아키텍처·단계 계획·완료 이력 |
| `PROJECTMAP.md` | 구조·줄 수·공용 유틸 추적 (1,000줄 = 구조 리팩터링 트리거) |
| `TODO.md` | 단계적 구현 대기열(파일 동기화 후속 등) |
| `CLAUDE.md` | Claude Code용 저장소 안내 |
| `AGENTS.md` | 에이전트 문서 지도 |
| `.github/copilot-instructions.md` | **코딩 지침 단일 정본** |
