# PROJECTMAP — 구조 · 크기 · 공용 유틸 추적

> 이 문서는 저장소의 **구조·크기·공용 유틸을 추적하는 단일 문서**다.
> 소스 파일을 추가·삭제·분할했거나 **헬퍼를 공용으로 승격했으면 같은 작업에서 이 문서를 갱신한다.**
> 규칙 정의는 `.github/copilot-instructions.md`의 「구조 리팩터링 기준(1,000줄 트리거)」과
> 「공용 유틸 승격 기준」 절을 정본으로 한다.
>
> **새 헬퍼를 만들기 전에 「공용 유틸 인벤토리」를 먼저 확인한다.**

**최종 측정**: 2026-07-29 · 총 29개 파일 · 7,883줄

## 크기 기준 — 줄 수는 증상이다

1,000줄 초과는 "파일이 길다"가 아니라 **책임 배치가 프로젝트 규모를 못 따라왔다**는 신호다.
따라서 조치는 파일 자르기가 아니라 **중복 제거 → 오배치 책임 이동 → (그래도 크면) 책임 단위 분할**
순서의 구조 리팩터링이다. ①~②만으로 임계값 아래로 내려가면 분할은 하지 않는다.

| 줄 수 | 상태 | 조치 |
| --- | --- | --- |
| ~800 | 🟢 정상 | — |
| 800~1,000 | 🟡 경고 | 다음 작업 전에 구조 리팩터링 |
| 1,000 초과 | 🔴 위반 | **즉시 리팩터링.** 다른 작업보다 우선 |

현재 🔴 위반 **없음**, 🟡 경고 **없음**. 최대 파일은 636줄(`window/service_mgr.rs`).
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
| `config.rs` | 406 | `AppConfig`·`SyncJob`·`LogConfig` 정의와 `update_config` 단일 저장 경로 |
| `sync.rs` | 460 | 폴더 동기화 엔진 (UI 비의존 순수 로직) |
| `logging.rs` | 431 | 롤링 파일 로거 (`log::Log` 구현) |

### 앱 루트 (`app/src/app/`) — 1,645줄 / 7파일

`app.rs`(1,798줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 572 | `AppRoot` 정의·생성자·사이드바·타이틀바·최상위 레이아웃(`impl Render`) |
| `sync_ops.rs` | 256 | 파일 동기화 작업 조작 (추가·삭제·선택·경로 적용·수동 실행) |
| `events.rs` | 202 | `PlatformEvent` 채널 소비, 로그·토스트 유틸 |
| `ops.rs` | 195 | 광고 차단·서비스 관리·로그 설정 조작 |
| `background.rs` | 170 | 스캔 스레드와 동기화 스레드 |
| `inputs.rs` | 129 | 입력 위젯(`InputState`) 지연 생성과 값 동기화 |
| `state.rs` | 121 | 순수 데이터 타입 (`AppState`, `PlatformEvent`, `ActivePanel` 등) |

### 패널 (`app/src/window/`) — 3,080줄 / 9파일

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `service_mgr.rs` | 636 | 편의 기능 — Windows 서비스 (목록/제어 ↔ 검색·필터·권한) |
| `settings.rs` | 568 | 전역 설정 — 테마 선택·로그 보관 정책 |
| `file_sync.rs` | 560 | 편의 기능 — 파일 동기화 (작업 목록·실패 기록 ↔ 작업 설정) |
| `ad_block.rs` | 466 | 편의 기능 — 웹뷰 광고 차단 (상태·타겟 ↔ 스캔 주기·프로세스 추가) |
| `service_view.rs` | 318 | 시스템 — 자동 시작(작업 스케줄러) 등록·삭제·즉시 실행 |
| `ui.rs` | 207 | **공용 UI 프리미티브** — 상태 배지·액션 버튼·대비 보장 토글 스위치 |
| `dashboard.rs` | 158 | 개요 — 전체 상태 요약과 최근 활동 |
| `log_view.rs` | 119 | 시스템 — 화면 로그 가상 리스트와 로그 파일 현황 |
| `mod.rs` | 48 | 패널 모듈 선언 + `scroll_pane` 헬퍼 |

### 플랫폼 (`app/src/platform/`) — 1,577줄 / 7파일

`windows.rs`(1,361줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 138 | `Platform` trait 정의 + 서비스 타입 (비Windows 기본 구현 포함) |
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

---

## 공용 유틸 인벤토리

**새 헬퍼를 만들기 전에 이 표를 먼저 본다.** 여기에 있는 것을 각 패널에서 다시 만들지 않는다.

| 소유 위치 | 항목 | 용도 |
| --- | --- | --- |
| `window/ui.rs` | `badge(label, Tone, Size, cx)` | 상태 배지. 폭 고정은 반환값에 `.w(px(..))` |
| `window/ui.rs` | `action_button(id, label, Size, ButtonStyle, on_click)` | 클릭 가능한 액션 버튼 |
| `window/ui.rs` | `toggle_switch(id, checked, cx)` | 테마와 상태에 맞는 대비 외곽선을 갖는 공용 스위치 |
| `window/ui.rs` | `Tone` | 배지 의미 색 — `Success`·`Warning`·`Info`·`Muted` |
| `window/ui.rs` | `ButtonStyle` | 버튼 의미 색 — `primary`·`neutral`·`secondary`·`danger`·`danger_outline`·`muted` (+ `border`/`hover`/`no_hover` 덮어쓰기) |
| `window/ui.rs` | `Size` | 여백 — `Sm`(px_2 py_1) · `Md`(px_3 py_1) · `Lg`(px_4 py_2) |
| `window/mod.rs` | `scroll_pane(id, handle, content)` | 자연 높이 컨텐츠가 넘칠 때 세로 스크롤 + 스크롤바 부여 |
| `theme.rs` | `change_theme` · `normalize_component_palette` | 테마 변경 후 스위치 트랙·썸 최소 대비 보정 |
| `config.rs` | `update_config(edit)` | 설정 읽기-수정-쓰기 **단일 경로** |
| `config.rs` | `data_dir` · `config_path` · `themes_path` · `logs_path` | `%APPDATA%` 하위 경로 계산 |
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
| 상태 배지 | ~~`ad_block::badge` · `service_view::state_badge` · `service_mgr` 인라인~~ | 🔴 3곳 | ✅ **승격 완료** → `ui::badge` (2026-07-29) |
| 액션 버튼 | ~~`file_sync::action_button` · `service_view::action_button` · `service_mgr` 인라인 6곳~~ | 🔴 3곳+ | ✅ **승격 완료** → `ui::action_button` (2026-07-29) |
| 스탯 타일 | `ad_block.rs stat_card` · `dashboard.rs stat_tile` | 🟡 2곳 | 후보 — `ui::stat_tile(label, value, cx)` |
| 토글 행 (설명 + `ui::toggle_switch`) | `file_sync.rs option_row` · `settings.rs` · `ad_block.rs` 인라인 | 🟡 2곳+ | 후보 — `ui::option_row(..)` |
| 선택 칩/옵션 행 | `settings.rs render_theme_option` · `render_filter_chip` · `service_mgr.rs` 필터 행 | 🟡 2곳+ | 후보 — 선택 상태를 인자로 받는 `ui::choice_row(..)` |
| 초 단위 간격 표기 | `file_sync.rs format_interval` · `ad_block.rs` 인라인 `format!("{}초")` 3곳 | 🟡 후보 | 후보 — `util.rs::format_interval(secs)` (UI가 아니므로 `ui.rs` 아님) |

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
| 2026-07-29 | `app.rs` | 책임 단위 분할 + 재배치 | 1,798 | `app/` 7파일 (최대 564) | 대시보드·로그 렌더는 소유가 잘못돼 있어 `window/`로 이동 |
| 2026-07-29 | `platform/windows.rs` | 책임 단위 분할 + 승격 | 1,361 | `platform/windows/` 6파일 (최대 344) | `wide_null`을 `windows/mod.rs`로 **공용 승격** |
| 2026-07-29 | 배지·액션 버튼 | **중복 제거 (공용 승격)** | 4파일 합 2,121 | 4파일 합 1,981 + `ui.rs` 194 | 정의 4개 + 인라인 8곳 → `ui.rs` 하나로 |
| 2026-07-29 | 테마·스위치·스크롤 | 공용 정책 승격 + 레이아웃 교정 | 35/36 테마에 스위치 토큰 없음 | 런타임 대비 보정 + 공용 스위치 + 36종 감사 | 파일 동기화 컨텐츠의 고정 높이를 제거해 자동 스크롤 복구 |

앞의 두 작업은 **동작 변경 없이** 수행했다. 세 번째 작업은 아래 한 건을 제외하면 화면이 동일하다.

- `service_view`의 등록/지금 실행 버튼이 `sidebar_primary` → `primary` 토큰으로 바뀌었다.
  사이드바 토큰을 패널 버튼에 쓰던 것이 의미 매핑 규칙 위반이라 승격하면서 교정했다.

세 작업 모두 `cargo check`(경고 0) · `cargo test`(12 passed / 1 ignored)로 확인했다.

**줄 수 총합은 55줄 늘었다**(`ui.rs`의 문서 주석 때문). 대신 패널 4개가 140줄 줄었다.
공용 승격의 목적은 총량 감소가 아니라 **변경 지점을 하나로 만드는 것**이다.

---

## 다음 리팩터링 후보

우선순위는 줄 수가 아니라 **구조 개선 효과** 순이다.

### 1순위 — 남은 중복 제거 (줄 수 무관, 지금 처리 가능)

🔴 항목은 처리했다. 남은 🟡 후보 중 **스탯 타일**과 **토글 행**이 다음 차례다.
셋 다 `ui.rs`에 흡수되며, 그만큼 아래 2순위 분할이 뒤로 밀린다. **그래서 순서가 중요하다.**

### 2순위 — 책임 재배치·분할 (중복 제거 후에도 크면)

- **`window/service_mgr.rs` (636)** — 가상 리스트 행 렌더가 큰 비중을 차지한다.
  800줄에 닿으면 행 렌더를 `service_mgr/row.rs`로, 보기 설정을 `service_mgr/settings.rs`로 분리한다.
- **`window/file_sync.rs` (563)** — `TODO.md`의 제외 패턴·진행률 UI가 들어가면 커진다.
  좌측(목록·실패 기록)과 우측(작업 설정)을 각각 별도 파일로 나누는 것이 자연스럽다.
  `format_interval`은 그 전에 `util.rs`로 승격한다.
- **`window/settings.rs` (542)** — 유일하게 이번 승격에서 줄지 않은 패널이다.
  선택 칩·테마 옵션이 자체 헬퍼로 남아 있어 🟡 「선택 칩/옵션 행」 승격 대상이며,
  기능별 설정이 흘러들어오지 않았는지(패널 구조 규약 위반) 함께 점검한다.
