# PROJECTMAP — 구조 · 크기 · 공용 유틸 추적

> 이 문서는 저장소의 **구조·크기·공용 유틸을 추적하는 단일 문서**다.
> 소스 파일을 추가·삭제·분할했거나 **헬퍼를 공용으로 승격했으면 같은 작업에서 이 문서를 갱신한다.**
> 규칙 정의는 `.github/copilot-instructions.md`의 「구조 리팩터링 기준(1,000줄 트리거)」과
> 「공용 유틸 승격 기준」 절을 정본으로 한다.
>
> **새 헬퍼를 만들기 전에 「공용 유틸 인벤토리」를 먼저 확인한다.**

**최종 측정**: 2026-07-30 · 총 38개 파일 · 11,422줄

## 크기 기준 — 줄 수는 증상이다

1,000줄 초과는 "파일이 길다"가 아니라 **책임 배치가 프로젝트 규모를 못 따라왔다**는 신호다.
따라서 조치는 파일 자르기가 아니라 **중복 제거 → 오배치 책임 이동 → (그래도 크면) 책임 단위 분할**
순서의 구조 리팩터링이다. ①~②만으로 임계값 아래로 내려가면 분할은 하지 않는다.

| 줄 수 | 상태 | 조치 |
| --- | --- | --- |
| ~800 | 🟢 정상 | — |
| 800~1,000 | 🟡 경고 | 다음 작업 전에 구조 리팩터링 |
| 1,000 초과 | 🔴 위반 | **즉시 리팩터링.** 다른 작업보다 우선 |

현재 🔴 위반 **없음**, 🟡 경고 **없음**. 최대 파일은 723줄(`app/mod.rs`).
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
| `config.rs` | 522 | `AppConfig`·`SyncJob`·`LogConfig`·주기 프리셋 정의, `update_config` 단일 저장 경로, 데이터 루트 오버라이드 |
| `logging.rs` | 560 | 롤링 파일 로거 (`log::Log` 구현, 테스트용 출력 경로 주입) |
| `util.rs` | 139 | 도메인 주인이 없는 순수 헬퍼 — `format_interval`·`interval_to_secs`·`TimeUnit` |

### 동기화 엔진 (`app/src/sync/`) — 1,059줄 / 2파일

`sync.rs`(831줄, 🟡)를 본문과 테스트로 나눈 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 545 | 폴더 동기화 엔진 (UI 비의존 순수 로직) — 진행 보고·중지·이어서 시작 제어 포함 |
| `tests.rs` | 514 | 복사·건너뜀·미러 삭제·실패 사유·진행 보고·중지·이어서 시작 단위 테스트 |

### 앱 루트 (`app/src/app/`) — 2,617줄 / 8파일

`app.rs`(1,798줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 723 | `AppRoot` 정의·생성자·백그라운드 UI wake·사이드바(전역 스위치 2개 포함)·최상위 레이아웃 |
| `sync_ops.rs` | 436 | 파일 동기화 작업 조작 (추가·삭제·선택·입력 저장·수동 실행 큐·중지·전역 스위치·커서 무효화) |
| `interval.rs` | 313 | 주기 선택 상태(`IntervalPicker`)와 조작 — 프리셋 추가·삭제·드롭다운 동기화 |
| `events.rs` | 271 | `PlatformEvent` 채널 소비, 진행 상태 반영, 로그·토스트 유틸 |
| `background.rs` | 320 | 스캔 스레드와 동기화 스레드 (진행 이벤트 빈도 제한·중지·실행 위치 영속화) |
| `state.rs` | 230 | 순수 데이터 타입 (`AppState`, `PlatformEvent`, `SyncRunning`, `ActivePanel`, 타깃별 `NAV_*`) |
| `ops.rs` | 195 | 광고 차단·서비스 관리·로그 설정 조작 |
| `inputs.rs` | 129 | 입력 위젯(`InputState`) 지연 생성과 값 동기화 |

### GPUI 회귀 테스트 (`app/src/app/tests/`) — 1,369줄 / 5파일

`app/tests.rs`(929줄, 🟡)를 시나리오별로 나눈 결과다. 픽스처는 `mod.rs`가 단독 소유하고
하위 모듈은 `use super::*`로 가져다 쓴다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `file_sync.rs` | 639 | 동기화 조작·진행 표시줄·중지·로그 요약·섹션 너비·전역 스위치·커서 무효화 |
| `layout.rs` | 251 | 사이드바·스플리터·divider drag·스크롤 |
| `interval.rs` | 226 | 주기 드롭다운·프리셋 추가/삭제·패널 간 공유 |
| `mod.rs` | 168 | 공용 픽스처 (`test_app_root`·`TestPlatform`·`refresh`·`click_debug_element` 등) |
| `theme.rs` | 85 | 테마 전환과 스위치 가시성 |

### 패널 (`app/src/window/`) — 3,244줄 / 10파일

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `service_mgr.rs` | 635 | 편의 기능 — Windows 서비스 (목록/제어 ↔ 검색·필터·권한) |
| `file_sync.rs` | 612 | 편의 기능 — 파일 동기화 (작업 목록 → 설정 → 실패 기록 + 하단 고정 진행 표시줄) |
| `settings.rs` | 444 | 전역 설정 — 테마 선택·로그 보관 정책 |
| `ad_block.rs` | 400 | 편의 기능 — 웹뷰 광고 차단 (상태·타겟 ↔ 스캔 주기·프로세스 추가) |
| `service_view.rs` | 318 | 시스템 — 자동 시작(작업 스케줄러) 등록·삭제·즉시 실행 |
| `ui.rs` | 322 | **공용 UI 프리미티브** — 배지·액션 버튼·토글 스위치·통계 타일·설정 행·선택 칩·로그 레벨 칸 |
| `dashboard.rs` | 161 | 개요 — 전체 상태 요약과 최근 활동 (플랫폼별 요약 카드 + 동기화 상태 배지) |
| `interval.rs` | 146 | 주기 선택 렌더 — 드롭다운 + (값·단위·추가) 행 + 등록된 프리셋 목록 |
| `log_view.rs` | 110 | 시스템 — 화면 로그 가상 리스트와 로그 파일 현황 |
| `mod.rs` | 96 | 패널 모듈 선언 + `balanced_split`·`scroll_pane` 레이아웃 헬퍼 |

### 플랫폼 (`app/src/platform/`) — 1,639줄 / 8파일

`windows.rs`(1,361줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 145 | `Platform` trait 정의 + 서비스 타입, `NativePlatform` 타깃별 별칭 |
| `fallback.rs` | 55 | 비Windows `Platform` 구현 — 광고 차단 계열 미지원을 명시적으로 반환 |
| `windows/scm.rs` | 344 | Windows 서비스(SCM) 등록과 서비스 모드 실행 |
| `windows/services.rs` | 340 | 설치된 Win32 서비스 조회·시작·중지·삭제, 권한 확인 |
| `windows/tray.rs` | 274 | 시스템 트레이 아이콘과 메시지 루프 |
| `windows/window_ops.rs` | 177 | 창·프로세스 열거, 광고 창 탐색 |
| `windows/mod.rs` | 156 | `WindowsPlatform` + `Platform` 구현, 하위 모듈 re-export |
| `windows/task_scheduler.rs` | 148 | 로그온 시 자동 시작(`schtasks`) |

### 빌드

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `app/build.rs` | 12 | `/MANIFEST:NO` 링커 인자 (gpui 임베드 매니페스트 중복 방지) |

### 개발·검증 도구

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `scripts/Verify-Workspace.ps1` | 166 | VS Code용 Rust/GPUI 자동 검증과 ChatGPT 데스크톱 handoff manifest·해시 고정 빌드 생성 |
| `scripts/Invoke-ClaudeVisualCheck.ps1` | 450 | `CLAUDE_LOCAL` 시각 검증 하네스 — 격리 실행(`-SeedConfig`로 상태 재현)·창 캡처(`PrintWindow`)·입력(`SendInput`)·정리 |
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
- **`app/tests/file_sync.rs` (639)** — 시나리오가 늘어 두 번째로 크다. 800줄에 닿으면
  레이아웃/조작/영속화로 다시 나눈다.
- **`window/service_mgr.rs` (635)** — 가상 리스트 행 렌더가 큰 비중을 차지한다.
  800줄에 닿으면 행 렌더를 `service_mgr/row.rs`로, 보기 설정을 `service_mgr/settings.rs`로 분리한다.
- **`window/file_sync.rs` (612)** — `TODO.md`의 제외 패턴·진행률 UI가 들어가면 커진다.
  작업 선택·설정·실패 기록의 렌더 책임을 의미별 하위 모듈로 나누는 것이 자연스럽다.
