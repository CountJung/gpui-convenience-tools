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
│       ├── config.rs            # AppConfig · SyncJob · LogConfig · 주기 프리셋 · update_config
│       ├── logging.rs           # 롤링 파일 로거 (log::Log 구현)
│       ├── util.rs              # 주인 없는 순수 헬퍼 (format_interval · interval_to_secs)
│       ├── sync/                # 폴더 동기화 엔진 (mod · tests)
│       ├── app/                 # AppRoot(Render) · 상태 · 이벤트 루프 · 사이드바
│       │   ├── mod.rs           #   구조체 · 생성자 · 최상위 레이아웃 · 전역 스위치
│       │   ├── state.rs         #   순수 데이터 타입
│       │   ├── background.rs    #   스캔 · 동기화 백그라운드 스레드 (실행 위치 영속화)
│       │   ├── ops.rs           #   광고 차단 · 서비스 · 로그 설정 조작
│       │   ├── sync_ops.rs      #   파일 동기화 작업 조작
│       │   ├── interval.rs      #   주기 선택 상태 · 프리셋 조작
│       │   ├── events.rs        #   백그라운드 → UI 이벤트 처리
│       │   ├── inputs.rs        #   입력 위젯 지연 생성
│       │   └── tests/           #   GPUI 회귀 테스트 5파일
│       ├── platform/
│       │   ├── mod.rs           # Platform trait (창 조작 + SCM)
│       │   ├── fallback.rs      # 비Windows 구현
│       │   └── windows/         # Win32 구현
│       │       ├── mod.rs       #   WindowsPlatform + Platform impl
│       │       ├── window_ops.rs#   창 · 프로세스 열거
│       │       ├── tray.rs      #   시스템 트레이
│       │       ├── scm.rs       #   Windows 서비스 등록 · 서비스 모드
│       │       ├── services.rs  #   설치된 서비스 조회 · 제어
│       │       └── task_scheduler.rs # 로그온 자동 시작
│       └── window/
│           ├── mod.rs           # 패널 모듈 + balanced_split · scroll_pane 헬퍼
│           ├── ui.rs            # 공용 UI 프리미티브 (배지 · 버튼 · 스위치 · 칩 …)
│           ├── ad_block.rs      # 편의 기능: 웹뷰 광고 차단
│           ├── file_sync.rs     # 편의 기능: 파일 동기화
│           ├── service_mgr.rs   # 편의 기능: Windows 서비스
│           ├── interval.rs      # 주기 선택 렌더 (두 패널 공용)
│           ├── dashboard.rs     # 개요: 전체 상태 요약
│           ├── log_view.rs      # 시스템: 화면 로그
│           ├── service_view.rs  # 시스템: 자동 시작(작업 스케줄러)
│           └── settings.rs      # 시스템: 전역 설정(테마 · 로그 보관)
└── installer/windows/
```

---

## 아키텍처 원칙

### 1. 편의 기능 = 독립 패널 + 작업 흐름에 맞는 레이아웃

각 편의 기능은 독립 패널을 유지하되 조작 흐름에 맞춰 레이아웃을 고른다.

| 형태 | 적용 기준 |
| --- | --- |
| 독립 탐색형 스플리터 | 목록과 설정을 동시에 비교하고 각각 독립 스크롤해야 하는 기능 |
| 전체 너비 연속형 | 작업 선택·설정·실행·결과 확인이 하나의 순서로 이어지는 기능 |

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

### 파일 동기화·로그 롤링 실사용 안정성 검증 ✅

- 사용자 데이터와 분리된 임시 디렉터리에서 3,000개 파일을 3.657초(약 820 files/sec)에
  동기화하고 결과 3,000건·실패 0건 확인
- 공유를 허용하지 않고 연 `open.xlsx`가 `공유 위반(code 32)`로 기록되고, 핸들 해제 후
  정상 복사되는 복구 경로 확인
- `attrib +h +s` 파일이 포함 설정에서는 복사되고 제외 설정에서는 건너뛰도록
  SYSTEM 속성까지 필터 판정에 포함
- 260자 초과 경로는 현재 Windows/Rust 환경에서 정상 복사됨을 확인하고, OS에서 거부할
  경우를 위한 `code 206` 사용자 사유 매핑 회귀 테스트 유지
- `RollingLogger`에 테스트용 로그 디렉터리 주입 경로를 추가하고 1MB 용량 롤링,
  현재 파일 포함 최대 3개 보존, 1일 초과 파일 삭제를 실제 파일로 검증

---

## 검증 재개 단계

### Phase J — 테마 가시성·스크롤 안정화 ✅

실제 화면 검증은 Phase N에서 마무리했다(아래 「Phase J / J-3 — 실제 화면 검증 완료」).

- 사이드바 그룹과 개별 항목에 `sidebar_border` 기반 경계를 적용해 비활성 항목 구분 강화
- 번들 테마 36개 변형의 스위치 팔레트를 감사하고, 누락 토큰을 런타임 최소 대비 정책으로 보정
- `window::ui::toggle_switch`로 스위치 외곽선 정책을 공용화하고 직접 `Switch::new` 사용 제거
- 파일 동기화 좌·우 컨텐츠의 고정 높이(`size_full`/세로 `flex_1`)를 제거해 자동 스크롤 복구
- 광고 차단·파일 동기화·서비스 관리의 고정 초기 폭을 공용 `balanced_split`으로 교체해
  기능·설정 pane이 가용 너비를 함께 사용하고 설정 영역 최소 300px을 유지하도록 개선
- 사이드바 전체를 독립 `scroll_pane`으로 감싸 작은 창에서도 마지막 시스템 항목까지 도달
- GPUI 자체 테스트로 기본 1000×700·최소 폭 920px의 pane 너비와 920×480의 사이드바
  wheel 스크롤·마지막 항목 도달을 검증
- GPUI 자체 테스트로 1000×700에서 기본 light/dark와 `Alduin` 테마의 실제 스위치
  off→on→off 전이, 920×480에서 파일 동기화 좌·우 pane wheel 스크롤·마지막 항목 도달을 검증
- 표준 `target\release`에서 `cargo build --release` 링크 완료
- Codex·Claude 공용 정본과 GPUI 스킬에 테마·스크롤 회귀 방지 규칙 반영
- Computer Use health check와 구현 담당자·독립 Visual Reviewer의 순차 크로스체크 계약 및
  Codex·Claude Code 에이전트 연결 추가
- `TestAppContext`/`VisualTestContext` 기반 수용 기준별 GPUI 회귀 테스트를 모든 UI 변경의
  필수 완료 조건으로 정본·스킬·Codex/Claude 브리지에 반영
- 과거 VS Code에서 Computer Use native pipe를 반복 확인하던 흐름을 폐기하고 실행 표면
  하드 게이트를 도입. VS Code/Claude Code는 wrapper를 초기화하지 않고
  `IDE_VERIFIED / DESKTOP_PENDING`으로 정상 인계
- `scripts/Verify-Workspace.ps1`와 VS Code 작업으로 GPUI 개별 테스트·전체 테스트·릴리즈
  빌드·SHA-256 manifest 생성을 단일화하고, ChatGPT 데스크톱은 해시 고정 바이너리와
  격리 `GPUI_CONVENIENCE_TOOLS_DATA_DIR` 프로세스만 검증하도록 분리
- 실제 화면 1차·2차 검증은 Phase J-2의 `CLAUDE_LOCAL` 하네스로 대체됐다
  (ChatGPT 데스크톱 인계는 `IDE` 표면에서만 사용)

### Phase J-3 — 앱 셸 리사이즈·파일 동기화 조작 흐름 개선 ✅

- 앱 셸 좌측 사이드바를 200~360px 범위의 `h_resizable` pane으로 전환
- 파일 동기화의 작업 목록·설정·실패 기록을 전체 너비 단일 스크롤 페이지로 통합
- 실행 버튼이 현재 이름·원본·대상 입력을 먼저 저장하고 선택 작업 ID를 실행 큐에 등록하도록
  변경했으며, 요청 상태를 즉시 표시
- 백그라운드 이벤트 채널에 완료 이벤트가 도착하면 GPUI 렌더를 깨워 결과가 추가 조작 없이
  갱신되도록 보완
- GPUI 테스트에서 사이드바 divider drag, 920×480 단일 페이지 overflow/마지막 기록 도달,
  실행 버튼의 입력 저장·공유 상태·큐 등록, 백그라운드 완료 이벤트의 무입력 UI 반영을 검증
- 실제 화면 검증은 Phase N에서 `CLAUDE_LOCAL` 하네스로 완료

### Phase J-2 — Claude Code 로컬 시각 검증 표면(`CLAUDE_LOCAL`) 도입 ✅

기존 하드 게이트는 ChatGPT 데스크톱의 Computer Use를 전제로 설계돼, 그 API가 아예 없는
Claude Code까지 `IDE`로 묶어 실제 화면 검증을 금지하고 있었다. 실측으로 대안 경로를 확인해
표면을 분리했다.

- Claude Code에는 Computer Use 도구가 존재하지 않음을 확인(전체 도구 목록 조회). ChatGPT에서
  문제가 됐던 wrapper·native pipe 오작동은 Claude Code에서 발생할 수 없다
- `PrintWindow(..., PW_RENDERFULLCONTENT)`가 GPU 렌더링(GPUI/Blade) 창을 정상 캡처하고,
  `SendInput` 휠·클릭이 대상 창에 전달되는 것을 실제 앱으로 확인. 전체 데스크톱이 아니라
  대상 창만 캡처된다
- `scripts/Invoke-ClaudeVisualCheck.ps1` 추가 — 격리 실행·캡처·휠·클릭·크기 변경·정리를
  단일 세션 모델로 제공. 포그라운드 확보 실패 시 입력을 보내지 않고 중단
- **격리 결함 수정**: `config::data_dir()`이 `dirs::config_dir()`(= `SHGetKnownFolderPath`)를
  쓰기 때문에 기존 검증 스크립트의 `APPDATA` 격리는 **한 번도 동작한 적이 없었고**, 검증
  프로세스가 사용자의 실제 `config.json`·로그를 그대로 사용했다. `GPUI_CONVENIENCE_TOOLS_DATA_DIR`
  오버라이드를 추가하고 두 시작 스크립트에 연결해 실제 격리를 확인
- **정리 실패 수정**: `Stop-DesktopVisualValidation.ps1`의 시작 시각 비교가 `ConvertFrom-Json`이
  만든 `Kind=Unspecified` DateTime을 다시 로컬로 해석해, UTC가 아닌 시간대에서는 항상
  9시간(KST) 어긋나 프로세스를 정리하지 못했다. 두 스크립트 모두 UTC 변환 헬퍼로 교체
- 정본·Claude 어댑터·공용 리뷰어 프롬프트·`CLAUDE.md`·`AGENTS.md`·VS Code 작업에 표면 분리와
  격리 규칙 반영

### Phase D-2 — 동기화 진행 표시·중지 ✅

`TODO.md`의 D-2(진행률 표시) 중 사용자 요청 범위를 구현했다.

- `sync.rs`에 `SyncControl`·`SyncProgress` 도입 — 엔진을 UI에 의존시키지 않기 위해 채널이
  아니라 콜백과 원자 플래그만 받는다. `run_sync_job_with_control`이 단일 진입점이다
- 파일 단위로 진행을 보고하고 중지 요청을 확인. 중지된 순회는 `seen_names`가 불완전하므로
  **미러 삭제를 실행하지 않는다**(확인하지 않은 원본의 대상본을 지우는 것을 막기 위함)
- 파일 동기화 패널 **하단에 고정 진행 표시줄** 추가 — 현재 파일·상태 배지·복사/건너뜀/실패
  카운터. 스크롤 영역 밖에 두어 스크롤 위치와 무관하게 항상 보인다
- **중지 버튼**을 '전체 지금 동기화' 옆에 배치. 실행 중에만 위험 색으로 강조하고,
  누르면 실행 중 작업과 대기 중인 수동 요청 큐를 함께 중단한다
- **동기화 로그를 파일 단위에서 실행 단위로 축소** — 백그라운드의 실패 1건당 `log::warn!`
  루프와 UI 로그의 실패 1건당 `ERROR` 항목을 제거하고, 실행마다 요약 한 줄만 남긴다.
  개별 실패 사유는 패널의 실패 목록이 소유한다
- 진행 이벤트는 120ms 간격으로 제한. 엔진은 파일마다 보고하지만 그대로 채널에 흘리면
  대용량 폴더에서 렌더 루프가 진행 표시만 그리게 된다
- GPUI 자체 테스트 4종·엔진 테스트 3종 추가. 실제 앱에서 12,000개 파일로 진행 표시·중지를
  검증(중지 시 2,067/12,000에서 멈추고 `복사 2067건 … — 중지됨` 기록)

### Phase K — macOS 빌드와 태그 기반 양 플랫폼 릴리즈 ✅

`v*` 태그 push 하나로 Windows와 macOS 앱을 모두 받을 수 있게 했다.

- **컴파일 블로커 해소**: `NativePlatform`이 Windows에서만 정의돼 다른 OS에서는 빌드가
  아예 불가능했다. `platform/fallback.rs`를 추가해 비Windows 별칭을 연결했다. 광고 차단
  계열은 성공을 가장하지 않고 미지원을 명시적으로 반환한다
- **macOS 구성 축소**: Win32/SCM 전용 패널(광고 차단·Windows 서비스·자동 시작)을
  `NAV_TOOLS`·`NAV_SYSTEM`에서 cfg로 제외하고, 사이드바 광고 차단 스위치와 대시보드의
  광고 차단 카드도 뺐다. 동작하지 않는 메뉴를 남기면 앱이 고장 난 것처럼 보이기 때문이다
- `installer/macos/build-app.sh` — `lipo` 유니버설 바이너리(arm64+x86_64), `Info.plist`,
  ad-hoc 코드 서명, DMG 생성. **arm64는 서명이 아예 없으면 실행되지 않으므로** 미서명
  배포라도 ad-hoc 서명은 반드시 붙인다
- `release.yml` — Windows·macOS 병렬 빌드 후 **둘 다 성공했을 때만** 단일 Release 생성.
  한쪽만 올라간 릴리즈가 생기면 사용자가 받은 버전을 알 수 없다
- `macos-build.yml` — push/PR마다 macOS check·test·패키징
- 코드 서명·공증은 하지 않는다. Gatekeeper 우회 안내를 릴리즈 노트와 README에 넣었다

**검증 제약(중요)**: Windows 개발 환경에서는 `#[cfg(not(target_os = "windows"))]` 경로를
컴파일할 수 없다. `--target`을 지정한 `cargo check`는 의존성 build script가 크로스 C
컴파일러(`x86_64-linux-gnu-gcc`)를 찾지 못해 실패한다. 그래서 macOS 경로의 검증은 전적으로
`macos-build.yml`에 의존하며, 태그를 만들기 전에 그 워크플로가 초록인지 확인해야 한다.
Windows 쪽은 34개 테스트 통과와 실제 앱 캡처로 회귀 없음을 확인했다.

### Phase L — 🟡 경고 해소 리팩터링 ✅

`PROJECTMAP.md`가 추적하던 리팩터링 대상을 정본의 순서대로 처리했다
(**① 중복 제거 → ② 오배치 이동 → ③ 책임 단위 분할**, ②는 대상 없음).

- **① 공용 승격** — `ui::stat_tile`·`ui::option_row`·`ui::choice_chip` 신설.
  통계 타일은 색 전달 방식만 다른 정의 2개, 토글 행과 프리셋 칩은 각각 **3개 파일**에
  같은 스타일 체인이 복제돼 있었다(즉시 승격 기준). 원본 정의는 모두 삭제
- **③ 분할** — `sync.rs`(831) → `sync/{mod,tests}.rs`,
  `app/tests.rs`(929) → `app/tests/{mod,layout,theme,file_sync}.rs`.
  ①만으로는 두 파일이 임계값 아래로 내려가지 않아 진행했다
- 결과: 🟡 경고 **2건 → 0건**, 최대 파일 929 → 656줄. 패널 4개 212줄 감소
- **맵 오판 정정** — 「초 단위 간격 표기」는 중복이 아니었다. `file_sync::format_interval`은
  60초를 `1분`으로 접지만 `ad_block`은 프리셋에 60·120초가 있어 `60초`·`120초`로 표기해야
  한다. 합쳤다면 화면이 바뀌었을 것이므로 승격 대상에서 제외하고 사유를 코드에 남겼다
- 분할로 테스트 경로가 `app::tests::<name>` → `app::tests::<모듈>::<name>`으로 바뀌어
  `Verify-Workspace.ps1`의 필수 테스트 매칭이 깨졌다. 모듈 경로에 의존하지 않도록 고쳤다
- **동작 변경 없음** 확인: `cargo check` 경고 0, 34개 테스트 통과, 실제 앱 캡처로 광고 차단·
  설정 패널이 승격 전과 동일하게 그려지는 것을 확인

### Phase M — 주기 선택 드롭다운과 UI 통일 ✅

고정 프리셋 칩은 목록에 없는 주기를 쓸 방법이 아예 없었다. 드롭다운 + 사용자 정의 추가로
바꾸고, 그 김에 미뤄 두었던 칩 패딩·주기 표기 통일까지 함께 처리했다.

- `util.rs` 신설 — `format_interval`(`10초`·`1분`·`1분 30초`·`1시간`)과
  `interval_to_secs(값, 단위)`. **딱 떨어지지 않는 값도 자르지 않고** 큰 단위부터 이어 붙인다
- `config.interval_presets` 추가 — 기본값 **10초·30초·1분**. 광고 차단 스캔 주기와 파일 동기화
  감시 주기가 **같은 목록을 공유**한다. 한쪽에서 만든 주기를 다른 쪽에서 또 만들 이유가 없다
- `app/interval.rs`(상태·조작) + `window/interval.rs`(렌더)로 분리. 드롭다운에서 고르고
  (값 + 단위 드롭다운)으로 새 프리셋을 추가하며, 등록된 프리셋은 `×`로 삭제한다
- 안전장치: 1초~24시간 범위 검사, 숫자 아닌 입력·오버플로 거부, **마지막 프리셋은 삭제 불가**
  (드롭다운이 비면 고를 수 없게 된다), 저장된 주기가 프리셋에 없어도 드롭다운에 합쳐 표시
- **의도된 화면 변경 2건** — 리팩터가 아니라 기능 변경이라 함께 처리했다.
  - 주기 표기 통일: `ad_block`이 쓰던 `60초`·`120초`가 `1분`·`2분`으로 바뀐다.
    Phase L에서 "합치면 화면이 바뀐다"며 보류했던 항목을 사용자 결정에 따라 통일
  - 칩 패딩 통일: `settings`의 테마 항목(`p_2`)·필터 칩(`px_2 py_1`)을
    프로젝트 표준인 `ui::choice_chip`(`px_3 py_2`)으로
- 검증: 단위 테스트 5종 + GPUI 회귀 6종(기본 프리셋·추가·잘못된 입력·패널 간 공유·마지막
  프리셋 보호·프리셋 밖 현재값) 추가, 전체 45개 통과. 실제 앱에서 드롭다운을 열어 `1분`을
  고르고 `scan_interval_secs: 60`이 저장되는 것까지 확인

### Phase N — 이어서 동기화 · 전역 스위치 · 스크롤 너비 고정 ✅

사용자 스크린샷에서 나온 세 가지를 처리했다.

- **이어서 동기화** — 앱을 켤 때마다 원본 전체를 다시 훑어 「건너뜀」만 쌓이던 문제.
  - `SyncJob`에 `last_run_unix`·`resume_cursor` 추가. **엔진 계층이 소유**하고 UI는 쓰지 않는다.
    UI 스냅샷 저장이 덮어쓰지 않도록 `config::carry_over_engine_progress`로 디스크 값을 되살린다
  - 백그라운드가 시작할 때 `last_run_unix`로 주기를 이어받아, **재시작 직후 전부 실행되던
    동작이 사라졌다**
  - 실행 중 5초마다 위치를 config에 남긴다. 트레이에서 강제 종료돼도 다음 실행이 이어진다
  - 엔진: `read_dir` 결과를 이름순 정렬(순서가 실행마다 같아야 커서가 성립)하고
    `SyncControl::resume_from`으로 커서 앞 구간을 건너뛴다. 커서 항목 자체는 다시 처리한다
    (기록 시점이 "처리 직전"이라 완료 여부를 알 수 없음)
  - 안전장치: 건너뛴 항목도 `seen_names`에 넣어 **미러 삭제가 확인하지 않은 파일을 지우지
    않게** 했고, 원본·대상 경로가 바뀌면 커서를 버린다. 완주한 실행은 커서를 비운다
  - 실측: 커서 `icu_collections-…`에서 중지 후 재시작하니 `js-sys-…`부터 이어졌고
    건너뜀이 1건이었다(이어서 시작이 없으면 수만 건)
- **파일 동기화 전역 스위치** — 사이드바에 광고 차단과 같은 모양으로 추가(`config.sync_enabled`).
  자동 실행만 막고 수동 '지금 동기화'는 그대로 둔다. 스위치 렌더는
  `AppRoot::render_sidebar_switch`로 공용화했다. 대시보드에도 상태 배지를 추가해
  패널에 들어가지 않아도 동기화 여부가 보인다
- **스크롤 컨텐츠 너비 고정** — `scroll_pane`에 `overflow_x_hidden` 추가.
  세로만 스크롤로 열어 두면 가로가 `visible`로 남아 컨텐츠가 뷰포트 폭 대신 자기 내용 폭으로
  잡히는 상황이 생긴다. **이 증상은 재현하지 못했고**(신규 실행·리사이즈·최소화 복원·디버그
  빌드·스크롤·동기화 중·트레이 복원·넓은 창 8종 모두 정상), 스크린샷의 섹션별 폭 차이에서
  역산한 가설 기반 대응이다
- **가로 잘림 3건 수정** — 위 변경으로 드러났거나 이미 잘려 있던 것.
  `+ 추가` 버튼(고정 64px에 글자가 안 들어가 두 줄로 접힘) → 최소 폭 + `whitespace_nowrap`,
  로그 레벨 칸이 대시보드 64px·로그 패널 72px로 갈려 `SUCCESS`가 접히던 것 →
  `ui::log_level_label`로 승격(레벨 → 색 매핑 중복도 함께 제거)
- 검증: 테스트 45 → **53개** 통과(엔진 이어서 실행 4종, config 보존 1종, GPUI 3종),
  `cargo check --all-targets` 경고 0, 실제 앱에서 스위치 on/off·config 저장·이어서 실행·
  1500px 폭 레이아웃 확인

### Phase J / J-3 — 실제 화면 검증 완료 ✅

`TODO.md`에 남아 있던 「Phase J 실제 화면 순차 검증」 대기 항목을 정리했다. Phase J-2에서
`CLAUDE_LOCAL` 하네스가 생긴 뒤 D-2·L·M·N을 거치며 해당 수용 기준이 모두 실제 캡처로
확인됐다.

- 사이드바 경계·overflow 스크롤, 독립 탐색형 스플리터(광고 차단·서비스 관리),
  파일 동기화 전체 너비 단일 페이지·스크롤·실행 결과 — 실제 앱 캡처로 확인
- 테마별 스위치 — `rendered_switch_toggles_in_light_dark_and_missing_switch_token_theme`
- **하네스 한계**: divider 드래그와 키보드 텍스트 입력은 `Invoke-ClaudeVisualCheck.ps1`가
  지원하지 않는다. 두 항목은 GPUI 테스트
  (`sidebar_divider_drag_resizes_navigation_and_content`,
  `file_sync_run_button_saves_current_inputs_and_queues_selected_job`)로만 검증된 상태다
- 앞으로 실제 화면 검증은 대기열에 쌓지 않고 UI를 바꾼 작업에서 그때 수행한다
  (정본: copilot-instructions「실행 표면 하드 게이트」)

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
