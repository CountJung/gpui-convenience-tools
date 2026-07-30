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

## 0. 실제 화면 검증 대기

- [ ] **Phase J 실제 화면 순차 검증**
  캡처 범위에는 사이드바 경계·divider 리사이즈·overflow 스크롤, 독립 탐색형 스플리터,
  테마별 스위치, 파일 동기화 전체 너비 단일 페이지·스크롤·실행 결과를 포함한다.
  구현 담당자 1차 검증 후 독립 Visual Reviewer 2차 검증을 수행한다.
  - **Windows Claude Code(`CLAUDE_LOCAL`)**: 인계 없이
    `scripts/Invoke-ClaudeVisualCheck.ps1`으로 두 검증을 모두 수행하고 `PASS`/`FAIL`/
    `BLOCKED`를 판정한다. 하네스가 커버하지 못하는 항목(키보드 텍스트 입력 등)은 한계로
    분리해 적고 그 항목을 근거로 `PASS`를 내지 않는다.
  - **VS Code Codex·Copilot(`IDE`)**: Computer Use를 시도하지 않는다.
    `GPUI: Prepare ChatGPT desktop handoff`로 해시 고정 빌드를 준비해 Windows ChatGPT
    데스크톱 앱의 Work 또는 Codex로 인계한다. `DESKTOP_PENDING`은 정상 인계 상태이므로
    native pipe를 재시도하거나 `BLOCKED(surface)`를 반복 기록하지 않는다.

---

## 1. 파일 동기화 — Phase D

### D-1. 제외 패턴 (우선순위 높음)

- [ ] `SyncJob`에 `exclude_patterns: Vec<String>` 추가(`#[serde(default)]`)
- [ ] glob 매칭 구현 — 의존성 추가 없이 `*`, `?`, `**` 정도만 자체 구현하거나
      `glob` 크레이트 추가 검토
- [ ] `sync::sync_dir`에서 상대 경로 기준으로 매칭해 건너뛰기(`skipped`로 계상)
- [ ] 설정 영역에 패턴 입력 UI(줄바꿈 구분 멀티라인 입력)
- [ ] 단위 테스트: 패턴에 걸린 파일이 복사되지 않는지

### D-2. 진행률 바 (선행 완료: 진행 표시·중지는 Phase D-2에서 구현됨)

현재 진행 표시줄은 **누적 개수**만 보여준다. 전체 대비 비율을 내려면 총 파일 수를 알아야
한다. 아래는 그 확장분만 남긴 것이다.

- [ ] 1단계로 전체 파일 수를 세고 2단계에서 복사 — 사전 스캔 비용(대용량 폴더에서 두 번
      순회)을 감수할지 먼저 결정한다. 감수하지 않으면 이 항목 전체를 접는다
- [ ] `SyncProgress`에 `total` 추가하고 백그라운드 이벤트에 전달
- [ ] 작업 행 또는 하단 표시줄에 진행률 바(`gpui_component::progress`) 표시

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
(독립 패널 + 작업 흐름에 맞는 스플리터/단일 페이지 + `NAV_TOOLS` 등록 +
`fills_height` 추가).

- [ ] **클립보드 히스토리** — 텍스트/이미지 최근 N개, 고정(pin), 검색
- [ ] **프로세스 모니터** — CPU/메모리 상위 프로세스, 강제 종료
- [ ] **빠른 실행기** — 등록한 앱/폴더/스크립트를 단축키로 실행
- [ ] **스크린샷 도구** — 영역 캡처 후 클립보드/파일 저장

---

## 3. 구조 개선

- [ ] **`ButtonStyle` 덮어쓰기 정리** *(독립, 단독 커밋)*
      `PROJECTMAP.md`「승격 후 남은 덮어쓰기」 표의 버튼 3건을 기본값으로 통일한다.
      화면이 실제로 바뀌므로 **리팩터와 섞지 않는다**.

- [ ] **`AppRoot` 분할 검토**
  현재 모든 패널 상태를 `AppRoot`가 직접 소유해 필드가 25개를 넘었다.
  기능별 상태 구조체(`AdBlockState`, `SyncState`, `ServiceState`)로 묶는 것을 검토한다.
  *주의: gpui 엔티티 분리까지 갈지, 단순 구조체 묶음으로 둘지 먼저 결정할 것.*

- [ ] **macOS 네이티브 기능 확장** *(선행 완료: Phase K에서 빌드·릴리즈는 구성됨)*
  현재 macOS는 `platform/fallback.rs`로 컴파일과 파일 동기화만 지원한다.
  아래는 macOS에서 의미가 있을 때만 진행한다.
  - [ ] 메뉴 막대(status bar) 상주 — Windows 트레이에 대응
  - [ ] `launchd` 로그온 자동 시작 — Windows 작업 스케줄러에 대응
  - [ ] `.icns` 앱 아이콘 (현재 기본 아이콘으로 배포됨)
  - [ ] 코드 서명·공증 — Apple Developer 계정 확보 시

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
- [ ] `config.rs` — `update_config`가 다른 필드를 보존하는지 회귀 테스트
