# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 정체성

Rust + GPUI 기반 **다용도 데스크탑 보조 도구 모음**. 개별 기능이 아니라 *편의 기능을 담는 그릇*이
프로젝트의 정체성이므로, 새 기능은 기존 기능과 결합하지 않는 **독립 패널**로 추가한다.

현재 편의 기능은 웹뷰 광고 차단, 파일 동기화, Windows 서비스 관리 세 가지다.
크레이트 하나(`app/`)로 구성된 cargo workspace이며, 패키지명은 디렉터리명과 다른
`gpui-convenience-tools`다.

## 명령어

```powershell
cargo run   -p gpui-convenience-tools            # 실행
cargo check -p gpui-convenience-tools            # 코드 수정 후 필수
cargo test  -p gpui-convenience-tools -- --nocapture
cargo build -p gpui-convenience-tools --release  # 단계 완료 시 필수

# 단일 테스트
cargo test -p gpui-convenience-tools copies_nested_files -- --nocapture
# `#[ignore]` 테스트(KakaoTalk 실행 필요)까지 포함
cargo test -p gpui-convenience-tools -- --ignored

# MSI 생성 (WiX Toolset v3 필요)
.\installer\windows\build-installer.ps1
```

실행 플래그: `--tray`(창 없이 트레이로 시작, 300ms 후 숨김), `--service`(SCM 서비스 디스패처
모드로 진입 후 GUI 없이 종료).

> **릴리즈 빌드 주의**: 트레이에서 실행 중인 인스턴스가 `target\release\*.exe`를 잠그면
> `cargo build --release`가 `os error 5`로 실패한다. 앱을 먼저 종료할 것.

## 아키텍처

### 크레이트 배치

```text
Cargo.toml          workspace (members = ["app"], 모든 의존성은 [workspace.dependencies])
app/
  src/config.rs     AppConfig · SyncJob · LogConfig · update_config
  src/logging.rs    롤링 파일 로거 (log::Log 구현)
  src/sync.rs       폴더 동기화 엔진 (순수 로직, UI 비의존)
  src/app/          AppRoot — 유일한 Render 엔티티, 모든 패널 상태 소유
    mod.rs            구조체 · 생성자 · 사이드바 · 최상위 레이아웃
    state.rs          순수 데이터 타입      background.rs  스캔 · 동기화 스레드
    ops.rs            광고차단·서비스·로그   sync_ops.rs    동기화 작업 조작
    events.rs         이벤트 소비 · 토스트   inputs.rs      입력 위젯 지연 생성
  src/platform/     Platform trait + windows/ (Win32 구현)
    windows/          mod · window_ops · tray · scm · services · task_scheduler
  src/window/       패널 렌더 함수 + scroll_pane 헬퍼
  assets/themes/    include_str!로 바이너리에 임베드되는 테마 JSON 21종
```

**파일별 줄 수·책임과 공용 유틸 인벤토리는 [PROJECTMAP.md](PROJECTMAP.md)를 정본으로 한다.**

### 1,000줄 트리거 = 구조 리팩터링 (단순 파일 분할이 아님)

**줄 수는 원인이 아니라 증상이다.** 1,000줄 초과는 "파일이 길다"가 아니라 **책임 배치가
프로젝트 규모를 못 따라왔다**는 신호이므로, 그때 하는 일은 파일 자르기가 아니라
**기능 관점의 구조 재설계**다. 줄 수만 맞추는 기계적 분할(`part1.rs`/`part2.rs` 류)은 금지.

트리거된 파일만 보지 말고 **프로젝트 전체에서 같은 책임이 어디에 흩어져 있는지 먼저 확인**한 뒤,
아래 순서로 처리한다. 1~2단계에서 임계값 아래로 내려가면 3단계는 하지 않는다.

1. **중복 제거 → 공용 유틸 승격** (아래 절)
2. **오배치 책임 이동** — 순수 로직은 도메인 모듈로, 패널 렌더는 `window/`로,
   Win32 호출은 `platform/windows/`로
3. **책임 단위 분할** — 같은 이름의 디렉터리 모듈로 전환(`app.rs` → `app/mod.rs`)

800줄부터 경고이고, 나중으로 미루지 않는다. 리팩터링은 **동작 변경 없이** 수행하며
`cargo check` + `cargo test`로 동일성을 확인하고 `PROJECTMAP.md`를 갱신한다.
상세 규칙은 copilot-instructions의 「구조 리팩터링 기준」 절이 정본이다.

### 공용 유틸 승격 (상시 적용)

1,000줄 트리거와 **무관하게 항상 적용된다.** 새 헬퍼를 만들기 전에 `PROJECTMAP.md`의
공용 유틸 인벤토리를 먼저 확인한다.

- **이름이 달라도 하는 일이 같으면 중복**이다(`badge`/`state_badge`,
  `stat_card`/`stat_tile`). 함수로 빼지 않은 인라인 스타일 체인 반복도 중복이다.
- 2개 파일에 있으면 승격 후보(맵에 기록), **3개 파일 이상이면 즉시 승격**.
  한 파일 안에서 같은 체인이 3회 이상이면 그 파일 안에서라도 헬퍼로 뺀다.
- **「즉시」 판정은 발견한 작업 안에서 끝낸다.** `TODO.md`로 미루지 않는다.
  대기열에 넣어도 되는 것은 후보(2곳)뿐이며, 즉시 항목을 미룰 때는 사유와 시점을
  `PROJECTMAP.md`에 남긴다.
- 승격 위치: GPUI 엘리먼트를 반환하면 `window/ui.rs`, 스크롤/레이아웃 래퍼는
  `window/mod.rs`, 엘리먼트를 반환하지 않는 순수 함수는 주인이 있는 도메인 모듈
  (`config`·`sync`·`logging`) 우선·없으면 `util.rs`, Win32 래퍼는
  `platform/windows/mod.rs`.
- 승격한 UI 헬퍼는 색상을 인자로 받지 말고 `cx.theme()`를 직접 읽는다.
  호출부마다 토큰을 넘기면 의미 매핑이 흩어진다.
- 승격 후 **원본 정의는 반드시 삭제한다.**

### 편의 기능 패널 구조 (중요)

각 편의 기능 페이지는 `h_resizable` 스플리터로 좌우가 나뉜다.

| 영역 | 내용 |
| --- | --- |
| 왼쪽 | 기능 본체 — 목록, 상태, 실행 결과 |
| 오른쪽 | **그 기능에만 해당하는** 설정 |

전역 설정 페이지([window/settings.rs](app/src/window/settings.rs))에는 앱 전체에 걸친 것
(테마, 로그 보관)만 둔다. **기능별 설정을 전역 설정에 넣지 않는다.**

새 편의 기능 추가 시 손봐야 할 지점:

1. `ActivePanel`에 variant 추가
2. `NAV_TOOLS`에 (패널, 명칭, 한 줄 설명) 추가 — 명칭은 기능을 모르는 사용자도 이해 가능하게
3. `window/<기능>.rs`에 `pub fn render(this: &mut AppRoot, window, cx) -> AnyElement`
4. 각 split 영역을 `window::scroll_pane`으로 감싸 독립 스크롤 부여
5. `render`의 패널 match 분기 + **`fills_height` 목록에 추가**
   (스플리터는 높이를 스스로 채우므로 바깥 스크롤을 걸면 안 됨)

스크롤 컨텐츠 루트는 자연 높이를 유지해야 한다. `size_full`/`h_full`이나 높이를 먹는
`flex_1().min_h_0()`를 긴 목록·설정 카드에 적용하면 스크롤 범위가 뷰포트에 고정되어
하단 내용이 잘릴 수 있다. 너비만 채울 때는 `w_full`을 사용한다. 상세 규칙은
copilot-instructions의 「스크롤 컨텐츠 높이 규칙」 절이 정본이다.

### 상태 흐름 — 양방향 규약

```text
       UI (AppRoot)
         │  ▲
 공유    │  │ PlatformEvent 채널
 뮤텍스  ▼  │ (백그라운드 → UI 단방향)
   ScannerState        SyncSharedState
         │                   │
   광고 스캔 스레드       동기화 스레드
   (tokio current_thread)  (std::thread, 1초 틱)
```

- **UI → 백그라운드**는 `Arc<Mutex<…>>`로만 전달. 상태를 바꿨으면 `sync_scanner_state()`
  또는 `persist_sync_jobs()`를 호출해야 백그라운드에 반영된다.
- **백그라운드 → UI**는 `PlatformEvent` 채널로만 전달. UI 이벤트 핸들러도 상태를 직접
  고치지 않고 `event_tx.send(...)` 후 `process_pending_events(window, cx)`를 호출한다.
  채널 드레인은 `render` 진입 시점에도 일어난다.
- 예외: 서비스 관리 패널만 동기 SCM 호출이 필요해 `this.platform.*`를 직접 호출한다.
- 파일 I/O는 블로킹이므로 동기화 스레드가 광고 스캔 스레드와 분리되어 있다.

### 설정 저장 — 단일 경로 필수

**저장은 반드시 `config::update_config(|cfg| ...)`를 사용한다.** 읽기-수정-쓰기를 한 곳에
모아둔 이유는, 예전에 저장 지점이 3곳으로 흩어져 각자 남의 필드를 손으로 복사하던 탓에
필드를 추가할 때마다 값이 유실됐기 때문이다. 개별 지점에서 `AppConfig`를 직접 구성하지 말 것.
새 필드에는 `#[serde(default)]`를 붙여 기존 config.json 호환성을 지킨다.

저장 위치는 모두 `%APPDATA%\gpui-convenience-tools\`: `config.json`, `themes/`, `logs/`.

### Platform 추상화

[platform/mod.rs](app/src/platform/mod.rs)의 `Platform: Send + Sync` trait이 창 조작과 SCM
조작을 모두 정의한다. 서비스 관련 메서드는 **비Windows에서 빈 결과나 에러를 반환하는 기본
구현**을 가지므로 새 플랫폼 추가 시 전부 구현할 필요는 없다. `platform/macos.rs`는 아직 없다.

Win32 구현은 [platform/windows/](app/src/platform/windows/) 아래에 책임별로 나뉘어 있고
(`window_ops` · `tray` · `scm` · `services` · `task_scheduler`) `#[cfg(target_os = "windows")]`로
게이트된다.

주의할 동작: `is_target_running`은 프로세스 존재가 아니라 `find_ad_window`가 WebView 자식 창을
찾았는지로 판정한다. 프로세스 열거도 `EnumWindows` 기반이라 **창이 없는 프로세스는 목록에
나오지 않는다**.

### 동기화 엔진

[sync.rs](app/src/sync.rs)는 UI에 의존하지 않는 순수 로직이라 단위 테스트가 가능하다(5종).
동기화 작업은 목록 인덱스가 아니라 **`SyncJob::id`로 식별**한다 — 인덱스는 작업 추가·삭제로
밀리기 때문에 백그라운드의 실행 주기 추적(`last_run`)과 결과 매칭(`sync_status`)이 어긋난다.
구버전 config에는 id가 없으므로 로드 직후 `ensure_id()`로 백필한다.
복사 판정은 `(크기, 수정 시각)` 비교이며 FAT 계열 타임스탬프 정밀도를 고려해 2초 여유를 둔다.
숨김·시스템 파일 포함이 기본값이고, 실패한 파일은 건너뛰되 사유를 `SyncFailure`로 모아
UI(토스트 + 로그 + 실패 목록)에 전달한다.

실패 알림은 세 겹으로 억제된다: 이미 목록에 있는 실패는 토스트 재표시 안 함 → 항목별
`알림 끄기` → 전체 `실패 알림 표시` 스위치.

### 로깅

[logging.rs](app/src/logging.rs)가 `log::Log`를 직접 구현해 콘솔과 파일에 동시 기록한다
(`env_logger` 미사용). 파일은 개수·기간·용량 3중 보존 기준을 함께 적용하며, 설정 변경은
`logging::update_config`로 즉시 반영된다.

`RUST_LOG`는 콘솔 레벨을 제어하고, 파일은 기본적으로 `Debug`까지만 받는다
(`DEFAULT_FILE_LEVEL`). **Trace를 파일에 허용하면 gpui의 프레임 단위 vsync 로그가 파일을 채워
실제 앱 로그가 롤링으로 밀려난다** — 이 상한을 풀 때는 그 점을 감안할 것.
(상한 도입 전후 비교: 20초 idle 기준 로그 파일 1,047B → 321B.)

### Windows 빌드 제약

- [build.rs](app/build.rs)는 `/MANIFEST:NO`만 지정한다. `gpui`의 `windows-manifest` 피처가
  이미 `RT_MANIFEST ID=1`을 임베드하므로 `winres`/`embed-resource`로 매니페스트를 추가하면
  `CVT1100` / `LNK1123` 중복 리소스 오류가 난다. `gpui`에 `default-features = false`를 줘도
  `gpui-component`가 기본 피처로 다시 켜므로 소용없다.
- 그 결과 `app/resources.rc`와 `app/*.exe.manifest`의 UAC `requireAdministrator` 설정은
  **현재 빌드에 반영되지 않는다**. 관리자 권한이 필요하면 수동으로 관리자 실행해야 하며,
  런타임 `is_elevated()` 확인 + 안내 배너로 처리한다. (`TODO.md`에 정리 항목 있음)
- `main.rs`의 `windows_subsystem = "windows"`는 `not(debug_assertions)` 조건부다. 조건을 떼면
  디버그 빌드에서 콘솔 로그 출력이 사라지고, 릴리즈에서 빼면 터미널을 닫을 때
  `CTRL_CLOSE_EVENT`로 트레이 앱이 죽는다.

### 자동 시작: 서비스가 아니라 작업 스케줄러

SCM 서비스는 Session 0(비대화형)에서 돌기 때문에 사용자 세션의 창을 조작할 수 없다.
자동 시작은 `schtasks /Create ... /SC ONLOGON /IT`로 처리한다("자동 시작" 탭).
SCM 코드(`install_win_service` 등)는 남아 있지만 이 용도로는 쓰지 않는다.

## 코딩 규칙

규칙 단일 정본은 [.github/copilot-instructions.md](.github/copilot-instructions.md)다. 핵심:

- **색상은 반드시 `cx.theme().<토큰>`**. 하드코딩 색상값 금지. 의미 매핑 고정: 페이지 배경
  `background`, 텍스트 `foreground`, 주요 액션 `primary`, 카드 표면 `secondary`/`list`,
  사이드바 `sidebar*`, 위험 `danger`. **`card`·`destructive`는 사용하지 않는다**.
- 스위치는 `window::ui::toggle_switch`, 테마 모드 변경은 `crate::theme::change_theme`를
  사용한다. 번들 테마 변경 시 전체 테마 변형의 스위치 대비 테스트를 실행한다.
- GPUI 0.2.2 / gpui-component 0.5.1 호환 API만 사용한다.
- `h_flex`/`v_flex` + `gap_*` 중심, 렌더 트리는 얕게. 렌더 경로에 비즈니스 로직 금지,
  UI 코드에서 `unwrap()` 지양. 클릭 가능한 `div`에는 `.id()`가 필요하다.
- 플랫폼 종속 코드는 `platform/` 아래에 두고 `#[cfg(target_os = "windows")]`를 유지한다.
- **외부 라이브러리(GPUI, gpui-component, windows-sys) API를 쓰기 전에 context7 MCP로 문서를
  조회한다.** `resolve-library-id` → `query-docs`. gpui-component는 `/longbridge/gpui-component`.
  단, context7 문서는 main 브랜치 기준이라 0.5.1과 다를 수 있으므로 **시그니처 확인은
  `~/.cargo/registry`의 로컬 크레이트 소스가 우선**이다.
- 코드 수정 후 `cargo check`, 단계 완료 시 `cargo build` + `cargo test`.
- GPUI UI 변경은 정본의 「GPUI 자체 테스트 컨텍스트 필수 검증」과
  「GPUI 시각 검증 및 독립 크로스체크」를 적용하고, 독립 검증에는
  [.claude/agents/ui-visual-reviewer.md](.claude/agents/ui-visual-reviewer.md)를 사용한다.
- 작업 범위는 `MasterPlan.md` 단계 기준. **완료 이력은 `MasterPlan.md`「구현 완료 단계」,
  미착수 대기열은 `TODO.md`** 두 곳만 쓴다(체크리스트를 복제하던 `TASKS.md`는 제거됨).
- 사용자에게 보이는 텍스트(설명, 주석, 커밋 메시지)는 한국어 또는 영어만 사용한다.

## CI

`.github/workflows/windows-build.yml` — main/master push 및 PR에서 check → test → release build.
`.github/workflows/release.yml` — `v*` 태그 push 시 릴리즈 빌드 + MSI를 Release 에셋으로 업로드.
