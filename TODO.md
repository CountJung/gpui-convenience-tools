# TODO — 단계적 구현 대기열

> 이 문서는 **아직 구현되지 않은 작업**만 담는다.
> 끝낸 항목은 여기서 지우고, 결과는 `MasterPlan.md`「구현 완료 단계」에 한 단계로 적는다.
> 단계 배경과 아키텍처 원칙은 `MasterPlan.md`, 코딩 규칙은 `.github/copilot-instructions.md`.
>
> **「즉시」 판정 항목(3개 파일 이상 중복, 1,000줄 초과)은 여기에 넣지 않는다** —
> 발견한 작업 안에서 처리한다. 이 문서에 들어오는 것은 후보와 미착수 기능뿐이다.

각 항목은 **한 턴/한 커밋 단위로 끝낼 수 있는 크기**로 쪼개 두었다.
`선행` 표시가 있으면 그 항목을 먼저 끝내야 한다.

---

## 0. 즉시 처리 (검증 잔여)

- [ ] **Phase J GPUI 자체 테스트 및 실제 화면 순차 검증**
  light/dark·이슈 테마의 on/off 스위치와 파일 동기화 좌·우 스크롤·마지막 항목 도달
  `TestAppContext`/`VisualTestContext` 회귀 테스트를 구현·통과시킨다. 이어서 Windows
  ChatGPT 데스크톱 앱의 Work 또는 Codex에서 구현 담당자 1차 검증 후 독립 Visual Reviewer
  2차 검증을 수행한다. 캡처 범위에는 사이드바 경계·overflow 스크롤, 균형 스플리터,
  테마별 스위치, 파일 동기화 스크롤을 포함한다. 파일 동기화는 임시 `APPDATA` 격리 환경을
  사용한다. VS Code의 Codex IDE 확장은 실제 화면 검증 표면으로 사용하지 않는다.

- [ ] **릴리즈 빌드 링크 확인**
  실행 중인 인스턴스가 `target\release\gpui-convenience-tools.exe`를 잠그면
  `cargo build --release`가 `os error 5`로 실패한다.
  트레이 앱을 종료(또는 `schtasks /End /TN "gpui-convenience-tools"`)한 뒤 재빌드해 확인한다.
  *`cargo check --release`는 통과 상태이므로 코드 문제는 아니다.*

- [ ] **파일 동기화 실사용 검증**
  - 대용량 폴더(수천 개 파일)에서 1회 동기화 소요 시간 측정
  - 사용 중인 파일(예: 열려 있는 xlsx)에서 `공유 위반(code 32)` 사유가 뜨는지 확인
  - 숨김/시스템 속성 파일이 실제로 복사되는지 확인(`attrib +h +s`로 테스트 파일 생성)
  - 260자 초과 경로에서 `code 206` 사유가 뜨는지 확인

- [ ] **로그 롤링 실사용 검증**
  용량을 1MB로 낮추고 로그를 대량 발생시켜 `app-YYYYMMDD-HHMMSS.log` 생성과
  개수·기간 초과분 삭제가 동작하는지 확인한다.

---

## 1. 파일 동기화 — Phase D

### D-1. 제외 패턴 (우선순위 높음)

- [ ] `SyncJob`에 `exclude_patterns: Vec<String>` 추가(`#[serde(default)]`)
- [ ] glob 매칭 구현 — 의존성 추가 없이 `*`, `?`, `**` 정도만 자체 구현하거나
      `glob` 크레이트 추가 검토
- [ ] `sync::sync_dir`에서 상대 경로 기준으로 매칭해 건너뛰기(`skipped`로 계상)
- [ ] 설정 영역에 패턴 입력 UI(줄바꿈 구분 멀티라인 입력)
- [ ] 단위 테스트: 패턴에 걸린 파일이 복사되지 않는지

### D-2. 진행률 표시

- [ ] `SyncProgress { job_index, current, total, current_path }` 이벤트 추가
- [ ] `sync::run_sync_job`에 진행 콜백 파라미터 추가
      (기존 시그니처 유지를 위해 `run_sync_job_with_progress` 분리 검토)
- [ ] 1단계로 전체 파일 수를 세고 2단계에서 복사 — 또는 누적 개수만 표시
- [ ] 패널 좌측 작업 행에 진행률 바(`gpui_component::progress`) 표시
- [ ] 실행 중 작업에 `취소` 버튼 — `Arc<AtomicBool>` 취소 플래그를 엔진에 전달

### D-3. 실시간 감시 (선행: D-2)

- [ ] `notify` 크레이트 사용(이미 의존성 트리에 존재 — 직접 의존성으로 승격 필요)
- [ ] `SyncJob`에 `watch_mode: WatchMode { Interval, Realtime }` 추가
- [ ] 파일 변경 이벤트를 디바운스(기본 2초)해 부분 동기화 실행
- [ ] 감시 실패(경로 없음, 핸들 한도 초과) 시 주기 모드로 자동 강등 + 로그
- [ ] 설정 영역에 감시 방식 선택 UI

### D-4. 심볼릭 링크 / 정션 처리

- [ ] 현재는 무조건 건너뛰고 실패로 기록 — 이를 옵션화
- [ ] `SyncJob`에 `symlink_mode: { Skip, Follow, Recreate }` 추가
- [ ] `Recreate`는 Windows에서 관리자 권한 또는 개발자 모드 필요 — 권한 없으면 사유 기록
- [ ] `Skip`일 때는 실패가 아니라 `skipped`로 계상하도록 변경(현재 동작 개선)

### D-5. 동기화 이력

- [ ] 실행 결과를 `%APPDATA%\gpui-convenience-tools\sync-history.json`에 append
- [ ] 보존 정책은 로그와 동일한 방식(개수/기간) 적용
- [ ] 패널에 최근 실행 이력 목록(성공/실패 건수, 소요 시간)

---

## 2. 편의 기능 확장 — Phase E

새 기능은 `.github/copilot-instructions.md`의 **편의 기능 패널 구조 기준**을 따른다
(독립 패널 + 스플리터 + `NAV_TOOLS` 등록 + `fills_height` 추가).

- [ ] **클립보드 히스토리** — 텍스트/이미지 최근 N개, 고정(pin), 검색
- [ ] **프로세스 모니터** — CPU/메모리 상위 프로세스, 강제 종료
- [ ] **빠른 실행기** — 등록한 앱/폴더/스크립트를 단축키로 실행
- [ ] **스크린샷 도구** — 영역 캡처 후 클립보드/파일 저장

---

## 3. 구조 개선

- [ ] **`window/ui.rs` 2차 승격 — 남은 🟡 후보** *(1순위)*
      `PROJECTMAP.md`「중복 헬퍼 추적」의 후보들을 처리한다.
      - 스탯 타일 — `ad_block::stat_card` + `dashboard::stat_tile` → `ui::stat_tile(label, value, cx)`
      - 토글 행 — `file_sync::option_row` + `settings.rs`·`ad_block.rs` 인라인 → `ui::option_row(..)`
      - 선택 칩 — `settings::render_theme_option`·`render_filter_chip` + `service_mgr` 필터 행
      - `format_interval` — UI가 아니므로 `util.rs`로 분리(`ad_block`의 `format!("{}초")` 3곳 흡수)

      승격 헬퍼는 색을 인자로 받지 말고 `cx.theme()`를 직접 읽는다. 승격 후 원본은 삭제.

- [ ] **`ButtonStyle` 덮어쓰기 정리** *(선행: 위 항목 아님, 독립)*
      `PROJECTMAP.md`「승격 후 남은 덮어쓰기」 표의 3건을 기본값으로 통일한다.
      화면이 실제로 바뀌므로 **리팩터와 섞지 말고 단독 커밋**으로 처리한다.

- [ ] **`AppRoot` 분할 검토**
  현재 모든 패널 상태를 `AppRoot`가 직접 소유해 필드가 25개를 넘었다.
  기능별 상태 구조체(`AdBlockState`, `SyncState`, `ServiceState`)로 묶는 것을 검토한다.
  *주의: gpui 엔티티 분리까지 갈지, 단순 구조체 묶음으로 둘지 먼저 결정할 것.*

- [ ] **`platform/macos.rs` 추가**
  현재 파일이 없어 macOS에서는 `Platform` 기본 구현만 동작한다(창 조작 불가).
  최소한 컴파일과 파일 동기화는 동작하도록 stub을 만든다.

- [ ] **스플리터 폭 영속화**
  `ResizableState`의 `sizes()`를 `on_resize`에서 읽어 config에 저장하고 복원한다.
  현재는 앱 재시작 시 기본 폭으로 돌아간다.

- [ ] **UAC 매니페스트 결정**
  `app/resources.rc`와 `app/*.exe.manifest`는 현재 빌드에 반영되지 않는 죽은 파일이다.
  둘 중 하나를 택한다.
  - (a) 파일을 삭제하고 README에 "관리자 권한은 수동 실행" 명시 — *현재 문서는 이 방향*
  - (b) `embed-resource`로 임베드하되 `gpui`의 `RT_MANIFEST` 중복을 회피하는 방법 확보

---

## 4. 테스트

- [ ] `sync.rs` — 읽기 전용 대상 파일 덮어쓰기 테스트
- [ ] `sync.rs` — 숨김 속성 파일 포함/제외 테스트(Windows 전용, `attrib` 사용)
- [ ] `logging.rs` — 용량 초과 롤링 및 개수 초과 삭제 테스트(임시 디렉터리 주입 필요,
      현재 `logs_path()`가 하드코딩이라 테스트를 위해 경로 주입 구조로 리팩터 필요)
- [ ] `config.rs` — `update_config`가 다른 필드를 보존하는지 회귀 테스트
