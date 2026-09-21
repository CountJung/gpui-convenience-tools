# PROJECT MAP — 구조 · 크기 · 공용 유틸 추적

> 이 문서는 저장소의 **구조·크기·공용 유틸을 추적하는 단일 문서**다.
> 소스 파일을 추가·삭제·분할했거나 **헬퍼를 공용으로 승격했으면 같은 작업에서 이 문서를 갱신한다.**
> 규칙 정의는 `DEVELOPMENT_GUIDE.md`의 「구조 리팩터링 기준(1,000줄 트리거)」과
> 「공용 유틸 승격 기준」 절을 정본으로 한다.
>
> **새 헬퍼를 만들기 전에 「공용 유틸 인벤토리」를 먼저 확인한다.**

## 구조 — VirtualBox 오프라인 디스크 탐색

구현된 파일과 예정 경로를 함께 추적한다. 새 파일을 추가하는 같은 작업에서 줄 수와 책임을
실측해 이 문서의 현재 파일 목록으로 승격한다.

| 경로 | 책임 | 관련 작업 ID |
| --- | --- | --- |
| `app/src/virtual_disk/mod.rs` | **구현됨** — `GuestFileSource`·파일 항목·오류 타입·백엔드 경계 | VDE-003 |
| `app/src/virtual_disk/vdi.rs` | **구현됨** — VDI 1.1 read-only 블록 리더와 잠금·VM 사용·원본 안정성 가드 | VDE-004~005 |
| `app/src/virtual_disk/partition.rs` | **구현됨** — MBR·EBR·protective MBR·GPT 검색과 CRC·범위 검증 | VDE-006 |
| `app/src/virtual_disk/ntfs.rs` | **구현됨** — NTFS 디렉터리 열거·기본 데이터 스트림 읽기·속성 보존·항목별 오류 경계 | VDE-007~008 |
| `app/src/virtual_disk/path_policy.rs` | **구현됨** — 게스트 경로의 호스트 매핑, 예약 이름·길이·링크 추적 안전 정책 | VDE-009 |
| `app/src/virtual_disk/copy.rs` | **구현됨** — 선택 파일·폴더의 청크 복사와 건너뜀·덮어쓰기·새 이름 충돌 정책 | VDE-010; 속성·오류 알림은 VDE-011~012 |
| `app/src/virtual_disk/metadata.rs` | **구현됨** — 호스트 파일 속성·타임스탬프 적용과 구조화된 적용 실패 결과 | VDE-011~012 |
| `app/src/window/virtual_disk.rs` | VDI 선택·파티션·탐색·선택·단축키·진행 UI | VDE-013~017 |
| `app/src/app/virtual_disk_ops.rs` | 탐색기 상태·이벤트·복사 작업 조작 | VDE-013~016 |

안전 경계: 원본 VDI는 read-only로만 열고, 실행 중 VM의 VDI 직접 읽기는 구현하지 않는다.
실행 중 VM 지원은 후속 `GuestFileSource` 구현으로만 추가한다(VDE-021).

**최종 측정**: 2026-09-21 · `app/src` 총 45개 파일 · 17,024줄

## 크기 기준 — 줄 수는 증상이다

1,000줄 초과는 "파일이 길다"가 아니라 **책임 배치가 프로젝트 규모를 못 따라왔다**는 신호다.
따라서 조치는 파일 자르기가 아니라 **중복 제거 → 오배치 책임 이동 → (그래도 크면) 책임 단위 분할**
순서의 구조 리팩터링이다. ①~②만으로 임계값 아래로 내려가면 분할은 하지 않는다.

| 줄 수 | 상태 | 조치 |
| --- | --- | --- |
| ~800 | 🟢 정상 | — |
| 800~1,000 | 🟡 경고 | 다음 작업 전에 구조 리팩터링 |
| 1,000 초과 | 🔴 위반 | **즉시 리팩터링.** 다른 작업보다 우선 |

현재 🔴 위반 **없음**, 🟡 경고 **없음**. 최대 파일은 782줄(`virtual_disk/vdi.rs`).
줄 수와 무관하게 처리하는 중복 헬퍼는 아래 「중복 헬퍼 추적」에서 관리한다.

### 줄 수 측정 명령

```powershell
Get-ChildItem app/src -Recurse -Filter *.rs | ForEach-Object {
    [PSCustomObject]@{ Lines = (Get-Content $_).Count; File = $_.FullName }
} | Sort-Object Lines -Descending
```

```bash
wc -l $(find app/src -name '*.rs' | sort) | sort -rn
```

---

## 파일 목록

### 루트 (`app/src/`)

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `main.rs` | 129 | 진입점 — 로거 설치 → 테마 시드 → 윈도우 오픈, `--service`/`--tray` 플래그 분기 |
| `theme.rs` | 143 | 테마 모드 적용과 스위치 팔레트 최소 대비 보정·번들 테마 감사 테스트 |
| `config.rs` | 546 | `AppConfig`·`SyncJob`·`LogConfig`·주기 프리셋 정의, 동기화 제외 패턴 스키마, `update_config` 단일 저장 경로, 데이터 루트 오버라이드 |
| `logging.rs` | 560 | 롤링 파일 로거 (`log::Log` 구현, 테스트용 출력 경로 주입) |
| `util.rs` | 139 | 도메인 주인이 없는 순수 헬퍼 — `format_interval`·`interval_to_secs`·`TimeUnit` |

### VirtualBox 도메인 (`app/src/virtual_disk/`) — 4,097줄 / 7파일

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 455 | VDI·파티션·게스트 파일 항목 모델, 정규화된 게스트 경로·시간 메타데이터, `GuestFileSource`, 플랫폼 비의존 오류 계약 |
| `vdi.rs` | 782 | VDI 1.1 read-only 헤더·블록 맵·동적/고정 블록 읽기, 잠금·VM 사용·크기/mtime 안정성 가드 |
| `partition.rs` | 729 | read-only `PartitionSource` 경계, MBR·EBR·protective MBR·GPT 검색, 양쪽 CRC·LBA 범위 검증 |
| `ntfs.rs` | 738 | 파티션 범위 `Read + Seek` 어댑터, NTFS 3.1 디렉터리·기본 데이터 스트림 읽기, 압축·암호화·범위·손상 오류와 속성·시간 보존 |
| `path_policy.rs` | 334 | 절대 대상 루트 기준 게스트 경로 매핑, Windows 예약 이름·경로 길이 검증, 게스트·호스트 리파스 포인트/심볼릭 링크 추적 차단 |
| `copy.rs` | 660 | `GuestFileSource` 선택 파일·폴더의 설정 청크 복사, 대상 부모 생성, 건너뜀·덮어쓰기·새 이름 충돌 정책과 메타데이터 결과 집계 |
| `metadata.rs` | 398 | Windows 파일 속성·생성/접근/수정 시간과 비지원·권한 오류를 `MetadataFailure`로 수집 |

### 동기화 엔진 (`app/src/sync/`) — 1,246줄 / 2파일

동기화 엔진을 `sync/mod.rs` 본문과 `sync/tests.rs` 테스트로 나눈 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 650 | 폴더 동기화 엔진 (UI 비의존 순수 로직) — 진행 보고·중지·이어서 시작·제외 glob 적용 포함 |
| `tests.rs` | 596 | 복사·건너뜀·미러 삭제·실패 사유·진행 보고·중지·이어서 시작·제외 glob 단위 테스트 |

### 앱 루트 (`app/src/app/`) — 2,769줄 / 8파일

`app.rs`(1,798줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 729 | `AppRoot` 정의·생성자·백그라운드 UI wake·사이드바(전역 스위치 2개 포함)·최상위 레이아웃 |
| `sync_ops.rs` | 456 | 파일 동기화 작업 조작 (추가·삭제·선택·이름·경로·제외 패턴 입력 저장·수동 실행 큐·중지·전역 스위치·커서 무효화) |
| `interval.rs` | 313 | 주기 선택 상태(`IntervalPicker`)와 조작 — 프리셋 추가·삭제·드롭다운 동기화 |
| `events.rs` | 271 | `PlatformEvent` 채널 소비, 진행 상태 반영, 로그·토스트 유틸 |
| `background.rs` | 427 | 스캔 스레드와 동기화 스레드 (다중 광고 창 추적·복원·진행 이벤트 빈도 제한·중지·실행 위치 영속화) |
| `state.rs` | 230 | 순수 데이터 타입 (`AppState`, `PlatformEvent`, `SyncRunning`, `ActivePanel`, 타깃별 `NAV_*`) |
| `ops.rs` | 195 | 광고 차단·서비스 관리·로그 설정 조작 |
| `inputs.rs` | 136 | 입력 위젯(`InputState`) 지연 생성과 값 동기화 — 파일 동기화 제외 패턴 멀티라인 편집기 포함 |

### GPUI 회귀 테스트 (`app/src/app/tests/`) — 1,563줄 / 5파일

`app/tests.rs`(929줄, 🟡)를 시나리오별로 나눈 결과다. 픽스처는 `mod.rs`가 단독 소유하고
하위 모듈은 `use super::*`로 가져다 쓴다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `file_sync.rs` | 682 | 동기화 조작·진행 표시줄·중지·로그 요약·섹션 너비·전역 스위치·커서 무효화·제외 패턴 UI 경계·저장 연결 |
| `layout.rs` | 397 | 사이드바·스플리터·카드 경계·divider drag·스크롤 |
| `interval.rs` | 226 | 주기 드롭다운·프리셋 추가/삭제·패널 간 공유 |
| `mod.rs` | 172 | 공용 픽스처 (`test_app_root`·`TestPlatform`·`refresh`·`click_debug_element` 등) |
| `theme.rs` | 85 | 테마 전환과 스위치 가시성 |

### 패널 (`app/src/window/`) — 3,440줄 / 10파일

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `service_mgr.rs` | 672 | 편의 기능 — Windows 서비스 (목록/제어 ↔ 검색·필터·권한) |
| `file_sync.rs` | 630 | 편의 기능 — 파일 동기화 (작업 목록 → 설정·제외 패턴 → 실패 기록 + 하단 고정 진행 표시줄) |
| `settings.rs` | 444 | 전역 설정 — 테마 선택·로그 보관 정책 |
| `ad_block.rs` | 502 | 편의 기능 — 웹뷰 광고 차단 (상태·타겟 ↔ 스캔 주기·프로세스 추가·카드 경계) |
| `service_view.rs` | 318 | 시스템 — 자동 시작(작업 스케줄러) 등록·삭제·즉시 실행 |
| `ui.rs` | 341 | **공용 UI 프리미티브** — 배지·액션 버튼·토글 스위치·통계 타일·설정 행·선택 칩·로그 레벨 칸·폭 경계 |
| `dashboard.rs` | 161 | 개요 — 전체 상태 요약과 최근 활동 (플랫폼별 요약 카드 + 동기화 상태 배지) |
| `interval.rs` | 156 | 주기 선택 렌더 — 드롭다운 + (값·단위·추가) 행 + 등록된 프리셋 목록 |
| `log_view.rs` | 110 | 시스템 — 화면 로그 가상 리스트와 로그 파일 현황 |
| `mod.rs` | 106 | 패널 모듈 선언 + `balanced_split`·`scroll_pane` 레이아웃 헬퍼 |

### 플랫폼 (`app/src/platform/`) — 2,390줄 / 8파일

`windows.rs`(1,361줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 220 | `Platform` trait 정의 + 광고 창 후보 목록·상태 스냅샷·서비스 타입, `NativePlatform` 타깃별 별칭 |
| `fallback.rs` | 59 | 비Windows `Platform` 구현 — 광고 차단 계열 미지원을 명시적으로 반환 |
| `windows/scm.rs` | 449 | Windows 서비스(SCM) 등록과 서비스 모드 실행 |
| `windows/services.rs` | 340 | 설치된 Win32 서비스 조회·시작·중지·삭제, 권한 확인 |
| `windows/tray.rs` | 274 | 시스템 트레이 아이콘과 메시지 루프 |
| `windows/window_ops.rs` | 719 | 타겟 프로세스 트리·최상위/명시적 자식 창 열거, 클래스 필터, 광고 팝업 후보 탐색, 창 상태 캡처·0×0 축소·복원 |
| `windows/mod.rs` | 178 | `WindowsPlatform` + `Platform` 구현, 다중 창 후보·상태 API 연결, 하위 모듈 re-export |
| `windows/task_scheduler.rs` | 148 | 로그온 시 자동 시작(`schtasks`) |

### 빌드

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `app/build.rs` | 12 | `/MANIFEST:NO` 링커 인자 (gpui 임베드 매니페스트 중복 방지) |

### 개발·검증 도구

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `.github/agents/code-reviewer.agent.md` | 70 | 공통 Code Reviewer 역할 정본 — 읽기 전용 종합 검토·검증 증거·문서 정합성 |
| `.github/agents/docs-sync.agent.md` | 69 | 공통 Documentation Sync 역할 정본 — `docs/` 정본 동기화·작업 ID·구조 지도 |
| `.claude/agents/code-reviewer.md` | 12 | Claude용 Code Reviewer 얇은 어댑터 |
| `.claude/agents/docs-sync.md` | 11 | Claude용 Documentation Sync 얇은 어댑터 |
| `.codex/agents/code-reviewer.toml` | 9 | Codex용 Code Reviewer 얇은 어댑터 |
| `.codex/agents/docs-sync.toml` | 9 | Codex용 Documentation Sync 얇은 어댑터 |
| `scripts/Verify-Workspace.ps1` | 167 | VS Code용 Rust/GPUI 자동 검증과 ChatGPT 데스크톱 handoff manifest·해시 고정 빌드 생성 |
| `scripts/Invoke-ClaudeVisualCheck.ps1` | 450 | `CLAUDE_LOCAL` 시각 검증 하네스 — 격리 실행(`-SeedConfig`로 상태 재현)·창 캡처(`PrintWindow`)·입력(`SendInput`)·정리 |
| `scripts/Verify-AdWindowState.ps1` | 282 | 지정 PID와 앱 조상·자손의 최상위·선택적 자식 창 상태와 클래스 후보를 읽기 전용 점검(AD-002·AD-005 진단) |
| `scripts/Start-DesktopVisualValidation.ps1` | 126 | manifest 해시 검증 후 단일 임시 데이터 루트 격리 프로세스·세션 파일 생성과 실패 롤백 |
| `scripts/Stop-DesktopVisualValidation.ps1` | 75 | 기록된 검증 PID·시작 시각과 작업 전용 임시 루트만 검증 후 정리 |
| `.vscode/tasks.json` | 111 | IDE 전용 검증, Claude 로컬 시각 세션, ChatGPT 데스크톱 인계 준비 작업 |
| `installer/macos/build-app.sh` | 135 | macOS 유니버설 `.app` 번들·ad-hoc 서명·DMG 생성 (macOS에서만 실행) |
| `.github/workflows/release.yml` | 140 | `v*` 태그 → Windows·macOS 병렬 빌드 후 단일 Release 생성 |
| `.github/workflows/macos-build.yml` | 47 | push/PR마다 macOS check·test·패키징 — 비Windows cfg 경로의 **유일한** 검증 지점 |

> `Invoke-ClaudeVisualCheck.ps1`은 450줄이지만 분할하지 않는다. 하나의 Win32 시퀀스
> (P/Invoke 선언 → 세션 → 캡처 → 입력 → 정리)를 공유하고, 쪼개면 각 파일이 같은 `Add-Type`
> 블록과 세션 스키마를 중복 소유하게 되어 응집도가 깨진다.
> 정본의 「구조 리팩터링 기준 > 예외」 조항을 적용한다.

---

## 공용 유틸 인벤토리

**새 헬퍼를 만들기 전에 이 표를 먼저 본다.** 여기에 있는 것을 각 패널에서 다시 만들지 않는다.

| 소유 위치 | 항목 | 용도 |
| --- | --- | --- |
| `window/ui.rs` | `badge(label, Tone, Size, cx)` | 상태 배지. 폭 고정은 반환값에 `.w(px(..))` |
| `window/ui.rs` | `action_button(id, label, Size, ButtonStyle, on_click)` | 클릭 가능한 액션 버튼 |
| `window/ui.rs` | `toggle_switch(id, checked, cx)` | 테마와 상태에 맞는 대비 외곽선을 갖는 공용 스위치 |
| `window/ui.rs` | `stat_tile(label, value, cx)` | 숫자 하나를 강조하는 통계 타일(`flex_1` 포함) |
| `window/ui.rs` | `option_row(id, title, description, checked, on_click, cx)` | 제목·설명 + 토글 스위치 설정 행. `{id}-row`/`{id}` debug_selector 부여 |
| `window/ui.rs` | `choice_chip(id, label, selected, cx)` | 프리셋 선택 칩. 호출부가 `.on_click(cx.listener(..))`를 이어 붙인다 |
| `window/ui.rs` | `log_level_label(level, cx)` | 로그 한 줄의 레벨 칸. 레벨 → 색 매핑 포함, 폭 고정 + 줄바꿈 금지 |
| `window/ui.rs` | `Tone` | 배지 의미 색 — `Success`·`Warning`·`Info`·`Muted` |
| `window/ui.rs` | `ButtonStyle` | 버튼 의미 색 — `primary`·`neutral`·`secondary`·`danger`·`danger_outline`·`muted` (+ `border`/`hover`/`no_hover` 덮어쓰기) |
| `window/ui.rs` | `Size` | 여백 — `Sm`(px_2 py_1) · `Md`(px_3 py_1) · `Lg`(px_4 py_2) |
| `window/mod.rs` | `balanced_split(id, left_min, right_min, left, right)` | 양쪽 최소 폭을 보장하며 가용 너비를 균형 있게 채우는 공용 스플리터 |
| `window/mod.rs` | `scroll_pane(id, handle, content)` | 자연 높이 컨텐츠가 넘칠 때 세로 스크롤 + 스크롤바 부여. **가로는 잠근다**(`overflow_x_hidden`) |
| `config.rs` | `carry_over_engine_progress(stored, jobs)` | UI 스냅샷 저장 시 엔진이 기록한 `last_run_unix`·`resume_cursor`를 디스크에서 되살린다 |
| `sync/mod.rs` | `SyncControl::resume_from(path)` | 끊긴 순회를 그 상대 경로부터 이어서 시작 (앞 구간만 건너뜀) |
| `theme.rs` | `change_theme` · `normalize_component_palette` | 테마 변경 후 스위치 트랙·썸 최소 대비 보정 |
| `config.rs` | `update_config(edit)` | 설정 읽기-수정-쓰기 **단일 경로** |
| `config.rs` | `data_dir` · `config_path` · `themes_path` · `logs_path` | 데이터 루트 하위 경로 계산 (`GPUI_CONVENIENCE_TOOLS_DATA_DIR`로 재지정 가능) |
| `config.rs` | `default_interval_presets` · `normalize_interval_presets` | 주기 프리셋 기본값(10·30·60초)과 정규화(중복 제거·오름차순) |
| `util.rs` | `format_interval(secs)` | 초 → `10초`·`1분`·`1분 30초`·`1시간` 표기. **모든 주기 표시는 이것만 쓴다** |
| `util.rs` | `interval_to_secs(amount, unit)` · `TimeUnit` | 사용자 입력 (값, 단위) → 초. 범위 밖이면 보여줄 사유를 반환 |
| `sync.rs` | `SyncControl` · `SyncProgress` | 동기화 엔진의 진행 보고·중지 제어 (보고 빈도 제한은 호출자 몫) |
| `app/state.rs` | `SyncRunning::counters` · `display_path` | 진행 표시줄 문자열 — 경로는 앞을 줄이고 파일명을 남긴다 |
| `logging.rs` | `now_hms` · `current_log_file` · `log_dir_stats` | 시각 문자열, 로그 파일 현황 조회 |
| `platform/windows/mod.rs` | `wide_null(&str)` | Win32 UTF-16 널 종단 문자열 변환 |

### 승격 위치 규칙

| 성격 | 소유 모듈 |
| --- | --- |
| GPUI 엘리먼트를 반환하는 UI 프리미티브 | `window/ui.rs` |
| 스크롤/레이아웃 래퍼 | `window/mod.rs` |
| 엘리먼트를 반환하지 않는 순수 함수 | 도메인 모듈(`config`·`sync`·`logging`) 우선, 없으면 `util.rs` *(아직 없음)* |
| Win32 래퍼 | `platform/windows/mod.rs` |

패널 파일(`window/<기능>.rs`)에는 **그 기능 고유의 도메인 지식이 든 헬퍼만** 남긴다.

---

## 중복 헬퍼 추적

판정: 2개 파일 = 후보, **3개 파일 이상 = 즉시 승격**(발견한 작업에서 바로 처리),
한 파일 내 동일 체인 3회 이상 = 그 파일 안에서 추출.

| 중복 | 위치 | 판정 | 상태 |
| --- | --- | --- | --- |
| 필터 행 | `service_mgr.rs` 상태 필터(`px_3 py_2` + 개수 배지 2단 구성) | 🟢 1곳 | 칩과 형태가 달라 승격 대상 아님. 배지가 붙는 변형이 하나 더 생기면 재검토 |

현재 추적 중인 **승격 후보 없음**. 선택 칩 패딩 차이는 아래 「리팩터링 이력」의
2026-07-30 주기 UI 작업에서 `ui::choice_chip`으로 통일하며 해소했다.

**해소됨** — `ui::stat_tile`·`ui::option_row`·`ui::choice_chip`으로 승격하며 원본 정의를 모두
삭제했다. `ad_block.rs stat_card`(색 인자 6개)와 `dashboard.rs stat_tile`은 색 전달 방식만
달랐고, 토글 행은 `file_sync`·`settings`·`ad_block` **3개 파일**(즉시 승격 기준), 프리셋 칩은
`ad_block`·`file_sync`·`settings` **3개 파일**에 같은 스타일 체인이 복제돼 있었다.

**「초 단위 간격 표기」 판정 정정 (2026-07-30)** — 한때 "합치면 화면이 바뀌므로 승격 대상이
아니다"라고 적었으나, 이는 잘못된 기준이었다. **같은 값을 화면마다 다르게 찍는 것 자체가
결함**이므로 화면 변경은 보류 사유가 되지 않는다. `util::format_interval`로 통일했고,
이 판단 기준은 정본의 「구현 기준」 첫 항목에 규칙으로 올렸다.

**로그 레벨 칸** — 대시보드 64px·로그 패널 72px로 갈려 있었고 좁은 쪽에서 `SUCCESS`가 두 줄로
접혀 잘렸다. 레벨 → 색 매핑까지 두 곳에 복제돼 있어 `ui::log_level_label`로 승격했다(2곳이지만
한쪽이 실제로 깨져 있어 즉시 처리).

**이름이 다르다고 다른 헬퍼가 아니다.** `badge`/`state_badge`, `stat_card`/`stat_tile`처럼
한쪽은 색을 인자로 받고 한쪽은 `cx.theme()`에서 읽는 정도의 차이는 같은 헬퍼로 본다.
승격 시 **`cx.theme()`를 직접 읽는 형태**를 기본으로 하고(호출부마다 토큰을 넘기면 의미 매핑이
흩어진다), 구현이 미세하게 다르면(`px_2` vs `px_3`) 통일 대상을 정하되 **그 통일은 별도 커밋**으로
분리한다. 승격 후 **원본 정의는 반드시 삭제한다.**

### 승격 후 남은 덮어쓰기 (통일 대기)

`ButtonStyle`의 `border`/`hover`/`no_hover` 덮어쓰기는 **승격 전 화면을 그대로 두기 위한 것**이며,
그 자체가 통일 후보다. 덮어쓰기가 늘어나면 기본값이 잘못됐다는 신호다.

| 화면 | 덮어쓰기 | 원래 모습 | 통일안 |
| --- | --- | --- | --- |
| `file_sync` 버튼 8개 | `.hover(border)` / `.border(primary_hover)` / `.border(danger_active)` | 테두리 색 = hover 색 | 기본값(테두리 `border`, hover `secondary_hover`)으로 |
| `service_view` 버튼 4개 | `.border(t.border).no_hover()` | hover 반응 없음 | hover 추가 여부 결정 |
| `service_mgr` 시작 버튼 | `.border(border).hover(secondary_hover)` (비활성 시) | 비활성도 테두리 유지 | `ButtonStyle::muted` 기본값에 테두리를 넣을지 결정 |

---

## 측정 제외

| 대상 | 사유 |
| --- | --- |
| `app/assets/themes/*.json` | 데이터 파일 (테마 21종) |
| `Cargo.lock` | 자동 생성 |
| `target/` | 빌드 산출물 |

---

## 리팩터링 이력

| 날짜 | 대상 | 종류 | 이전 | 이후 | 비고 |
| --- | --- | --- | ---: | --- | --- |
| 2026-09-17 | 프로젝트 문서 | 정본 디렉터리 통합 | 루트 문서 5개 + 에이전트 규칙 중복 | `docs/` 5개 정본 + 루트 에이전트 어댑터 | 공통 내용은 `DEVELOPMENT_GUIDE.md`로 통합하고 어댑터에는 표면별 주의사항만 유지 |
| 2026-09-17 | `virtual_disk/mod.rs` | VDE-003 도메인 경계 추가 | VirtualBox 탐색 모델 없음 | 403줄의 플랫폼 비의존 모델·읽기 계약·오류 분류 | VDI 실제 파서와 GPUI 화면은 VDE-004 이후 연결 |
| 2026-09-21 | `virtual_disk/vdi.rs` | VDE-004 VDI 블록 리더 추가 | VDI 헤더·블록 맵 구현 없음 | 512줄의 read-only 헤더·맵 검증·동적/고정 블록 읽기 | 파티션·NTFS·잠금 안정성은 VDE-005~008에서 연결 |
| 2026-09-21 | `virtual_disk/vdi.rs` | VDE-005 안전 가드 추가 | 파일 잠금·원본 변경·실행 중 VM 대조 없음 | 782줄의 잠금 표식·환경변수 기반 VBoxManage 조회·크기/mtime 스냅샷·read-only 핸들 검증 | 800줄 주의 구간에 접근했으므로 VDE-006 전에 책임 단위 분할 후보를 검토 |
| 2026-09-21 | `virtual_disk/partition.rs` | VDE-006 MBR/GPT 파서 추가 | 파티션 검색 구현 없음 | 729줄의 MBR·EBR·GPT 양쪽 CRC·범위 검증과 `PartitionSource` 어댑터 | NTFS 파일시스템 판정은 VDE-007~008에서 연결 |
| 2026-09-21 | `virtual_disk/ntfs.rs` | VDE-007 NTFS 읽기 전용 어댑터 추가 | NTFS 디렉터리·스트림 접근 구현 없음 | 507줄의 파티션 범위 `Read + Seek`, NTFS 3.1 열거·기본 스트림 읽기·속성 보존과 env 주입 이미지 테스트 | 압축·암호화·항목별 오류 억제는 VDE-008에서 연결 |
| 2026-09-21 | `virtual_disk/ntfs.rs`·`virtual_disk/mod.rs` | VDE-008 NTFS 오류 경계 추가 | 파일 경로 없는 공통 오류와 압축·암호화 스트림 무방비 읽기 | `GuestEntry` 경로 래퍼, 파티션 범위 재검증, 압축·암호화 플래그 차단, 손상 메타데이터·원본 변경 보존, 오류 단위 테스트와 손상 이미지 테스트 | 항목별 복사 실패 알림·고정 손상 픽스처 묶음은 VDE-012·VDE-018에서 연결 |
| 2026-09-21 | `virtual_disk/path_policy.rs`·`virtual_disk/mod.rs` | VDE-009 호스트 경로 안전 매핑 추가 | 게스트 경로를 호스트 복사 대상에 연결하는 안전 경계 없음 | 절대 대상 루트, Windows 예약 이름·금지 문자·UTF-16 길이 제한, 게스트 리파스 포인트와 기존 호스트 링크 추적 차단, 6개 정책 테스트 | 실제 파일 복사·충돌·속성·오류 알림 연결은 VDE-010~012에서 수행 |
| 2026-09-21 | `virtual_disk/path_policy.rs`·`virtual_disk/copy.rs` | VDE-010 복사 엔진과 경로 정책 분리 | 경로 정책과 복사 엔진이 한 파일에 모여 952줄 경고 구간에 접근 | 경로 정책을 334줄 모듈로 분리하고 복사 엔진을 629줄로 유지, 청크 복사·부모 생성·건너뜀·덮어쓰기·새 이름 충돌과 13개 테스트 추가 | 파일 속성·항목별 실패 알림은 VDE-011~012에서 연결 |
| 2026-09-21 | `virtual_disk/metadata.rs`·`virtual_disk/ntfs.rs`·`virtual_disk/copy.rs` | VDE-011 메타데이터 적용 추가 | 게스트 속성과 시간 정보가 호스트 복사 결과에 반영되지 않음 | NTFS 생성·수정·접근 시간 모델링, Windows 속성·생성 시간·접근/수정 시간 적용, 비지원·권한 실패 수집, 5개 테스트 | 실제 VDI 이미지의 모든 메타데이터 조합과 UI 알림 연결은 VDE-018~019·VDE-012에서 후속 검증 |
| 2026-07-29 | 편의 기능 스플리터 3곳 | 공용 레이아웃 승격 | 패널별 고정 초기 폭 | `window::balanced_split` | 설정 pane 과도 축소 방지, 양쪽 가용폭 사용 |
| 2026-07-29 | `app.rs` | 책임 단위 분할 + 재배치 | 1,798 | `app/` 7파일 (최대 564) | 대시보드·로그 렌더는 소유가 잘못돼 있어 `window/`로 이동 |
| 2026-07-29 | `platform/windows.rs` | 책임 단위 분할 + 승격 | 1,361 | `platform/windows/` 6파일 (최대 344) | `wide_null`을 `windows/mod.rs`로 **공용 승격** |
| 2026-07-29 | 배지·액션 버튼 | **중복 제거 (공용 승격)** | 4파일 합 2,121 | 4파일 합 1,981 + `ui.rs` 194 | 정의 4개 + 인라인 8곳 → `ui.rs` 하나로 |
| 2026-07-29 | 테마·스위치·스크롤 | 공용 정책 승격 + 레이아웃 교정 | 35/36 테마에 스위치 토큰 없음 | 런타임 대비 보정 + 공용 스위치 + 36종 감사 | 파일 동기화 컨텐츠의 고정 높이를 제거해 자동 스크롤 복구 |
| 2026-07-30 | 앱 셸·파일 동기화 | 조작 흐름 재설계 | 고정 사이드바 + 파일 동기화 이중 pane | 리사이즈 사이드바 + 전체 너비 단일 스크롤 | 실행 시 현재 입력 자동 저장·완료 이벤트 UI wake 포함 |
| 2026-07-30 | 통계 타일·설정 행·프리셋 칩 | **중복 제거 (공용 승격)** | 4파일 합 1,933 | 4파일 합 1,721 + `ui.rs` 207→300 | 정의 2개 + 인라인 복제 다수 → `ui::stat_tile`·`option_row`·`choice_chip` |
| 2026-07-30 | `sync.rs` | 책임 단위 분할 | 831 🟡 | `sync/` 2파일 (435 / 399) | 본문과 단위 테스트 분리 |
| 2026-07-30 | `app/tests.rs` | 책임 단위 분할 | 929 🟡 | `app/tests/` 4파일 (최대 448) | 픽스처는 `mod.rs`가 단독 소유, 시나리오별 하위 모듈 |
| 2026-07-30 | 주기 선택 UI | **기능 재설계 + 통일** | 패널별 고정 프리셋 칩 | `app/interval.rs` + `window/interval.rs` + `util.rs` | 드롭다운 + 사용자 정의 프리셋. 칩 패딩·주기 표기도 함께 통일(화면 변경 의도됨) |
| 2026-07-30 | 사이드바 전역 스위치 | 공용 승격 | 광고 차단 전용 인라인 | `AppRoot::render_sidebar_switch` | 파일 동기화 스위치를 같은 모양으로 추가하며 승격 |
| 2026-07-30 | 로그 레벨 칸 | **중복 제거 (공용 승격)** | `dashboard` 64px / `log_view` 72px | `ui::log_level_label` (76px) | 좁은 쪽에서 `SUCCESS`가 접혀 잘렸다. 레벨→색 매핑 중복도 함께 흡수 |
| 2026-07-30 | `scroll_pane` | 레이아웃 고정 | 세로만 스크롤, 가로 `visible` | `overflow_x_hidden` 추가 | 컨텐츠가 뷰포트 폭 대신 내용 폭으로 잡히는 경로 차단(가설 기반) |

앞의 두 작업은 **동작 변경 없이** 수행했다. 세 번째 작업은 아래 한 건을 제외하면 화면이 동일하다.

- `service_view`의 등록/지금 실행 버튼이 `sidebar_primary` → `primary` 토큰으로 바뀌었다.
  사이드바 토큰을 패널 버튼에 쓰던 것이 의미 매핑 규칙 위반이라 승격하면서 교정했다.

세 작업 모두 `cargo check`(경고 0) · `cargo test`(12 passed / 1 ignored)로 확인했다.

**줄 수 총합은 55줄 늘었다**(`ui.rs`의 문서 주석 때문). 대신 패널 4개가 140줄 줄었다.
공용 승격의 목적은 총량 감소가 아니라 **변경 지점을 하나로 만드는 것**이다.

### 2026-07-30 리팩터링 (🟡 경고 해소)

정본의 순서대로 **① 중복 제거 → ③ 책임 단위 분할**을 수행했다. ②(오배치 이동)는 대상이
없었다. ①만으로는 `sync.rs`·`app/tests.rs`가 임계값 아래로 내려가지 않아 ③까지 진행했다.

- 패널 4개 212줄 감소(`ad_block` −57, `settings` −54, `file_sync` −42, `dashboard` −14),
  `ui.rs`는 93줄 증가. 결과적으로 🟡 경고 **2건 → 0건**, 최대 파일 929 → 656줄
- **동작 변경 없음**: `cargo check` 경고 0, 34개 테스트 통과, 실제 앱 캡처로
  광고 차단·설정 패널이 승격 전과 동일하게 그려지는 것을 확인
- 분할로 테스트 경로가 `app::tests::<name>` → `app::tests::<모듈>::<name>`으로 바뀌어
  `Verify-Workspace.ps1`의 필수 테스트 매칭을 모듈 경로에 의존하지 않도록 함께 고쳤다

---

## 다음 리팩터링 후보

우선순위는 줄 수가 아니라 **구조 개선 효과** 순이다.

### 1순위 — 남은 중복 제거 (줄 수 무관, 지금 처리 가능)

**추적 중인 승격 후보 없음.** 새로 발견하면 「중복 헬퍼 추적」 표에 먼저 적는다.

다만 **`debug_selector` 래퍼**는 관찰 대상이다. `ui::action_button`(`Stateful<Div>`)과
`ui::toggle_switch`(`Switch`)가 debug_selector를 달지 않아, 테스트에서 클릭해야 하는 곳마다
`div().debug_selector(..)`로 감싸는 한 줄이 14곳에 복제돼 있다. 헬퍼 안으로 옮기려면 `id`를
`impl Into<ElementId>`에서 문자열로 좁혀야 하는데, `("svc-start", ix)`처럼 튜플 id를 쓰는
호출부 3곳이 걸린다. **한 줄짜리 관용구라 지금은 승격하지 않는다.** id 체계를 손볼 일이
생기면 함께 처리한다.

### 2순위 — 책임 재배치·분할 (중복 제거 후에도 크면)

- **`app/mod.rs` (723)** — 최대 파일이며 800줄 경고까지 77줄 남았다. `AppRoot` 필드가 30개에
  가깝다. `TODO.md`의 「`AppRoot` 분할 검토」와 같은 항목이다. 다음에 커지면 사이드바 렌더를
  `app/sidebar.rs`로 떼는 것이 가장 자연스럽다.
- **`app/tests/file_sync.rs` (682)** — 시나리오가 늘어 두 번째로 크다. 800줄에 닿으면
  레이아웃/조작/영속화로 다시 나눈다.
- **`window/service_mgr.rs` (635)** — 가상 리스트 행 렌더가 큰 비중을 차지한다.
  800줄에 닿으면 행 렌더를 `service_mgr/row.rs`로, 보기 설정을 `service_mgr/settings.rs`로 분리한다.
- **`window/file_sync.rs` (630)** — 진행률·추가 설정 UI가 들어가면 커질 수 있다.
  작업 선택·설정·실패 기록의 렌더 책임을 의미별 하위 모듈로 나누는 것이 자연스럽다.
