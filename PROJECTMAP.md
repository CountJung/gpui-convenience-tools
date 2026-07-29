# PROJECTMAP — 구조 · 크기 · 공용 유틸 추적

> 이 문서는 저장소의 **구조·크기·공용 유틸을 추적하는 단일 문서**다.
> 소스 파일을 추가·삭제·분할했거나 **헬퍼를 공용으로 승격했으면 같은 작업에서 이 문서를 갱신한다.**
> 규칙 정의는 `.github/copilot-instructions.md`의 「구조 리팩터링 기준(1,000줄 트리거)」과
> 「공용 유틸 승격 기준」 절을 정본으로 한다.
>
> **새 헬퍼를 만들기 전에 「공용 유틸 인벤토리」를 먼저 확인한다.**

**최종 측정**: 2026-07-29 · 총 27개 파일 · 7,631줄

## 크기 기준 — 줄 수는 증상이다

1,000줄 초과는 "파일이 길다"가 아니라 **책임 배치가 프로젝트 규모를 못 따라왔다**는 신호다.
따라서 조치는 파일 자르기가 아니라 **중복 제거 → 오배치 책임 이동 → (그래도 크면) 책임 단위 분할**
순서의 구조 리팩터링이다. ①~②만으로 임계값 아래로 내려가면 분할은 하지 않는다.

| 줄 수 | 상태 | 조치 |
| --- | --- | --- |
| ~800 | 🟢 정상 | — |
| 800~1,000 | 🟡 경고 | 다음 작업 전에 구조 리팩터링 |
| 1,000 초과 | 🔴 위반 | **즉시 리팩터링.** 다른 작업보다 우선 |

현재 🔴 위반 **없음**, 🟡 경고 **없음**. 최대 파일은 690줄(`window/service_mgr.rs`).
다만 **줄 수와 무관하게 처리해야 할 중복 헬퍼가 남아 있다** — 아래 「중복 헬퍼 추적」 참조.

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
| `config.rs` | 400 | `AppConfig`·`SyncJob`·`LogConfig` 정의와 `update_config` 단일 저장 경로 |
| `sync.rs` | 460 | 폴더 동기화 엔진 (UI 비의존 순수 로직) |
| `logging.rs` | 431 | 롤링 파일 로거 (`log::Log` 구현) |

### 앱 루트 (`app/src/app/`) — 1,637줄 / 7파일

`app.rs`(1,798줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 564 | `AppRoot` 정의·생성자·사이드바·타이틀바·최상위 레이아웃(`impl Render`) |
| `sync_ops.rs` | 256 | 파일 동기화 작업 조작 (추가·삭제·선택·경로 적용·수동 실행) |
| `events.rs` | 202 | `PlatformEvent` 채널 소비, 로그·토스트 유틸 |
| `ops.rs` | 195 | 광고 차단·서비스 관리·로그 설정 조작 |
| `background.rs` | 170 | 스캔 스레드와 동기화 스레드 |
| `inputs.rs` | 129 | 입력 위젯(`InputState`) 지연 생성과 값 동기화 |
| `state.rs` | 121 | 순수 데이터 타입 (`AppState`, `PlatformEvent`, `ActivePanel` 등) |

### 패널 (`app/src/window/`) — 2,613줄 / 8파일

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `service_mgr.rs` | 690 | 편의 기능 — Windows 서비스 (목록/제어 ↔ 검색·필터·권한) |
| `file_sync.rs` | 585 | 편의 기능 — 파일 동기화 (작업 목록·실패 기록 ↔ 작업 설정) |
| `settings.rs` | 542 | 전역 설정 — 테마 선택·로그 보관 정책 |
| `ad_block.rs` | 474 | 편의 기능 — 웹뷰 광고 차단 (상태·타겟 ↔ 스캔 주기·프로세스 추가) |
| `service_view.rs` | 372 | 시스템 — 자동 시작(작업 스케줄러) 등록·삭제·즉시 실행 |
| `dashboard.rs` | 158 | 개요 — 전체 상태 요약과 최근 활동 |
| `log_view.rs` | 119 | 시스템 — 화면 로그 가상 리스트와 로그 파일 현황 |
| `mod.rs` | 45 | 패널 모듈 선언 + `scroll_pane` 헬퍼 |

### 플랫폼 (`app/src/platform/`) — 1,377줄 / 7파일

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
| `window/mod.rs` | `scroll_pane(id, handle, content)` | 스플리터 영역에 세로 스크롤 + 상시 스크롤바 부여 |
| `config.rs` | `update_config(edit)` | 설정 읽기-수정-쓰기 **단일 경로** |
| `config.rs` | `data_dir` · `config_path` · `themes_path` · `logs_path` | `%APPDATA%` 하위 경로 계산 |
| `logging.rs` | `now_hms` · `current_log_file` · `log_dir_stats` | 시각 문자열, 로그 파일 현황 조회 |
| `platform/windows/mod.rs` | `wide_null(&str)` | Win32 UTF-16 널 종단 문자열 변환 |

### 승격 위치 규칙

| 성격 | 소유 모듈 |
| --- | --- |
| GPUI 엘리먼트를 반환하는 UI 프리미티브 | `window/ui.rs` *(아직 없음 — 첫 승격 시 생성)* |
| 스크롤/레이아웃 래퍼 | `window/mod.rs` |
| 엘리먼트를 반환하지 않는 순수 함수 | 도메인 모듈(`config`·`sync`·`logging`) 우선, 없으면 `util.rs` *(아직 없음)* |
| Win32 래퍼 | `platform/windows/mod.rs` |

패널 파일(`window/<기능>.rs`)에는 **그 기능 고유의 도메인 지식이 든 헬퍼만** 남긴다.

---

## 중복 헬퍼 추적

실측 기준 아래 중복이 남아 있다. **줄 수 임계값과 무관하게** 승격 대상이다.
(판정: 2개 파일 = 후보, 3개 파일 이상 = 즉시 승격, 한 파일 내 동일 체인 3회 이상 = 파일 내 추출)

| 중복 | 현재 위치 | 판정 | 승격안 |
| --- | --- | --- | --- |
| 상태 배지 (`rounded_md` + `px` + `bg` + `text_color` + 라벨) | `ad_block.rs:440 badge` · `service_view.rs:75 state_badge` · `service_mgr.rs:192` 인라인 | 🔴 3곳 — 즉시 | `window/ui.rs::badge(label, tone, cx)` |
| 액션 버튼 | `file_sync.rs:530 action_button` · `service_view.rs:98 action_button` · `service_mgr.rs`/`settings.rs` 인라인 다수 | 🔴 3곳 — 즉시 | `window/ui.rs::action_button(id, label, tone, enabled, on_click)` |
| 스탯 타일 | `ad_block.rs:451 stat_card` · `dashboard.rs:141 stat_tile` | 🟡 2곳 — 후보 | `window/ui.rs::stat_tile(label, value, cx)` |
| 토글 행 (설명 + `Switch`) | `file_sync.rs:554 option_row` · `settings.rs:215` · `ad_block.rs:368` 인라인 | 🟡 2곳 이상 — 후보 | `window/ui.rs::option_row(...)` |
| 초 단위 간격 표기 | `file_sync.rs:577 format_interval` · `ad_block.rs:189/264/381` 인라인 `format!("{}초")` | 🟡 후보 | `util.rs::format_interval(secs)` |
| 테마 토큰 스냅샷 | `service_view.rs:28 ThemeSnap` | ⚪ 단일 | 승격 시 `window/ui.rs`가 `cx.theme()`를 직접 읽는 형태로 흡수 검토 |

**이름이 다르다고 다른 헬퍼가 아니다.** `badge`/`state_badge`, `stat_card`/`stat_tile`처럼
한쪽은 색을 인자로 받고 한쪽은 `cx.theme()`에서 읽는 정도의 차이는 같은 헬퍼로 본다.
승격 시 **`cx.theme()`를 직접 읽는 형태**를 기본으로 하고(호출부마다 토큰을 넘기면 의미 매핑이
흩어진다), 구현이 미세하게 다르면(`px_2` vs `px_3`) 통일 대상을 정하되 **그 통일은 별도 커밋**으로
분리한다. 승격 후 **원본 정의는 반드시 삭제한다.**

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

두 작업 모두 **동작 변경 없이** 수행했고, `cargo check`(경고 0) · `cargo test`(12 passed) ·
실행 스모크 테스트로 동일성을 확인했다.

---

## 다음 리팩터링 후보

우선순위는 줄 수가 아니라 **구조 개선 효과** 순이다.

### 1순위 — 중복 제거 (줄 수 무관, 지금 처리 가능)

`window/ui.rs`를 신설해 위 「중복 헬퍼 추적」의 🔴 항목(상태 배지·액션 버튼)을 먼저 흡수한다.
이 하나만으로 `service_mgr.rs`·`file_sync.rs`·`ad_block.rs`·`service_view.rs` 네 파일이 함께
줄어들어, 아래 2순위 분할이 필요 없어질 가능성이 있다. **그래서 순서가 중요하다.**

### 2순위 — 책임 재배치·분할 (중복 제거 후에도 크면)

- **`window/service_mgr.rs` (690)** — 가상 리스트 행 렌더가 큰 비중을 차지한다.
  공용 헬퍼 흡수 후에도 800줄에 닿으면 행 렌더를 `service_mgr/row.rs`로,
  보기 설정을 `service_mgr/settings.rs`로 분리한다.
- **`window/file_sync.rs` (585)** — `TODO.md`의 제외 패턴·진행률 UI가 들어가면 커진다.
  좌측(목록·실패 기록)과 우측(작업 설정)을 각각 별도 파일로 나누는 것이 자연스럽다.
  `format_interval`은 그 전에 `util.rs`로 승격한다.
- **`window/settings.rs` (542)** — 전역 설정 페이지. 기능별 설정이 흘러들어오지 않았는지
  (편의 기능 패널 구조 규약 위반) 리팩터링 시 함께 점검한다.
