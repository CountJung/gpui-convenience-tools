# 작업별 검증 실행·기록

> 이 문서는 코드 수정 결과를 작업 ID별로 검증하고 증거를 남기는 실행 정본이다.
> 공통 판정 기준은 `DEVELOPMENT_GUIDE.md`, GPUI 화면 절차는
> `.github/skills/gpui-visual-check/SKILL.md`를 참조한다. 이 문서에는 실행 순서와
> 작업별 체크 상태만 기록한다.

## 현재 E2E 경로

이 저장소에는 Playwright·브라우저용 E2E 하네스가 없다. 현재 사용자 흐름 검증은 다음 두
단계를 조합한다.

1. **GPUI 상호작용 수용 테스트** — `#[gpui::test]`에서 실제 대상 뷰를 렌더링하고
   `TestAppContext`/`VisualTestContext`로 리사이즈·클릭·키보드·wheel을 입력한 뒤 앱 상태,
   선택·포커스, 스크롤 offset, `debug_bounds`를 단언한다. 이것이 코드 수정의 기본 E2E
   대체 경로다.
2. **데스크톱 black-box 검증** — Windows에서 release 바이너리를 격리 실행하고
   `scripts/Invoke-ClaudeVisualCheck.ps1` 또는 데스크톱 handoff 절차로 실제 창 전환·스크롤·
   캡처를 확인한다. 캡처만으로 GPUI 자체 테스트를 대신하지 않는다.

텍스트 입력·divider drag처럼 로컬 창 하네스가 지원하지 않는 입력은 GPUI 테스트로 검증하고,
하네스 미지원 상태를 `N/A` 사유 없이 `PASS`로 기록하지 않는다.

최신 품질 게이트 기준은 `cargo check -p gpui-convenience-tools --locked`, 전체 테스트
155 passed·4 ignored, 표준 `cargo clippy --all-targets --all-features --locked` exit 0·경고
0건이다. 전체 `cargo fmt --check`는 기존 baseline 포맷 차이가 남아 있어 별도 구조 정리 범위로
유지한다.

## 작업마다 적용하는 순서

작업 ID 하나를 완료 처리하기 전에 아래 순서를 따른다. 문서 전용 작업은 해당 없는 코드
게이트를 `N/A`로 기록하고 문서 assertion을 대신 실행한다.

### 1. 범위와 검증 프로필 선택

| 프로필 | 적용 대상 | 필수 검증 |
| --- | --- | --- |
| `RUST` | 순수 Rust·플랫폼·설정·파일 I/O | `cargo check --locked` + 관련 테스트 + 전체 테스트(단계 완료 시) |
| `GPUI` | 레이아웃·테마·포커스·스크롤·사용자 입력 | 관련 `#[gpui::test]` + `cargo test --all-targets --all-features` |
| `E2E` | 여러 상태/화면을 잇는 사용자 흐름 | `GPUI` 수용 테스트 + 가능하면 격리 데스크톱 검증 |
| `DOCS` | 문서·TODO·프로젝트 맵 | `git diff --check` + 링크/ID assertion + 참조 경로 확인 |

### 2. 자동 검증 실행

```powershell
# 모든 코드 변경의 기본 게이트
cargo check -p gpui-convenience-tools --locked

# 관련 테스트를 먼저 실행한다. <test_name>은 작업의 수용 기준에 대응하는 실제 이름이다.
cargo test -p gpui-convenience-tools <test_name> -- --nocapture

# 단계 완료 또는 여러 모듈에 영향을 준 변경
cargo test --all-targets --all-features

# GPUI 필수 테스트 목록·전체 테스트·IDE 표면 판정을 한 번에 수행
pwsh -NoProfile -File .\scripts\Verify-Workspace.ps1
```

`Verify-Workspace.ps1`가 실패하면 작업을 완료로 체크하지 않는다. 실패가 기존 baseline인지
확인한 경우에도 원문 오류, 실행 명령, 영향 범위를 작업 기록에 남긴다.

### 3. GPUI E2E 수용 테스트 작성·실행

UI를 바꿀 때는 수용 기준마다 테스트 하나를 연결한다.

- 기본 창 크기와 최소 지원 창 크기를 모두 사용한다.
- 입력 전 상태, 입력(`simulate_click`, `simulate_keystrokes`, `simulate_event`, wheel),
  입력 후 상태와 기대 bounds를 단언한다.
- 안정적인 `debug_selector`를 부여하고 취약한 절대 좌표만으로 통과시키지 않는다.
- 앱이 단순히 panic 없이 렌더링된 것만으로는 통과로 보지 않는다.
- 새 테스트는 `scripts/Verify-Workspace.ps1`의 필수 목록에도 등록한다.

### 4. 실제 데스크톱 E2E/시각 검증

Windows Claude Code에서는 다음처럼 작업 전용 세션을 사용한다. 상세 좌표·시드·정리 규칙은
`.github/skills/gpui-visual-check/SKILL.md`만 참조한다.

```powershell
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
cargo build -p gpui-convenience-tools --release
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Start -Width 994 -Height 702
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name before
# 필요한 경우 Click 또는 Wheel 실행 후 반드시 다시 Capture
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
```

VS Code/Codex의 `IDE` 표면에서는 화면 조작을 시도하지 않고 다음으로 handoff를 준비한다.

```powershell
pwsh -NoProfile -File .\scripts\Verify-Workspace.ps1 -PrepareDesktopHandoff
```

`CLAUDE_LOCAL`/`DESKTOP` 검증은 캡처 PNG, 창 크기, 시나리오, 기대·관찰 결과, 바이너리 커밋과
SHA-256을 기록한다. 다른 검증자의 독립 검토가 요구되는 UI 작업은 구현 세션과 별도 세션에서
같은 시나리오를 반복한다.

### 5. 완료 체크와 커밋

작업 ID의 이 문서 행에서 적용 게이트를 모두 체크한 뒤에만 완료한다.

- `[B]` 빌드/정적 검사 통과
- `[T]` 관련 자동 테스트 및 필요한 경우 전체 테스트 통과
- `[E]` GPUI 상호작용 또는 데스크톱 E2E 통과
- `[V]` 실제 화면/시각 검증 통과 또는 명시적 `N/A` 사유
- `[R]` 오류 리뷰 완료 및 오류 없음/잔여 오류 기록
- `[C]` 검증 결과를 커밋하고 `origin/main`까지 푸시

`TODO.md`에서 항목을 삭제하는 시점은 `[B]`, `[T]`, `[R]`, `[C]`와 해당 작업의 `[E]`/`[V]`
게이트가 모두 끝난 뒤다. 완료 결과는 `MASTER_PLAN.md`에 한 번 기록하고, 테스트 이름·명령·
캡처 경로·커밋을 중복해서 여러 문서에 복사하지 않고 이 문서의 증거 칸에서 참조한다.

## 작업별 체크 매트릭스

`TODO.md`의 모든 활성 ID는 이 표에 정확히 한 번 있어야 한다. `—`는 작업 성격상 해당
게이트가 적용되지 않음을 뜻하고, 적용 게이트는 `[ ]`에서 `[x]`로 갱신한다.

| ID | 프로필 | B | T | E | V | R | C | 증거 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| VDE-012 | E2E | [x] | [x] | [x] | [ ] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 전체 `cargo test --all-targets --all-features --locked` 155 passed·4 ignored; 오류 행 selector·억제 키 저장/재로드·미억제 토스트 게이트와 `virtual_disk_panel_renders_copy_progress_and_issue_summary`의 GPUI `simulate_click` 억제/재표시 dispatch를 검증; 실제 VBox 생성 VDI 릴리스 복사에서 `many_subdirs` 손상 사유·17개 파일 부분 결과를 확인; 독립 Visual Reviewer와 실제 릴리스 버튼 조작은 잔여 |
| VDE-013 | GPUI | [x] | [x] | [x] | [x] | [x] | [ ] | `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (123 passed, 4 ignored); GPUI 테스트 `virtual_disk_panel_registers_navigation_and_renders_read_only_shell`; 릴리즈 `CLAUDE_LOCAL` 920×700·1280×700 캡처 `target/visual-validation/captures/vde013-panel-920-final-201106.png`, `vde013-panel-1280-final-201106.png`; 세션 정리 후 프로세스 0·임시 루트 0; 실제 VDI 성공 경로·파티션 파일 목록 E2E는 VDE-018 후속 |
| VDE-014 | GPUI | [x] | [x] | [ ] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (125 passed, 4 ignored); GPUI 셸 테스트에 상위 이동 액션 경계 포함; 릴리즈 `CLAUDE_LOCAL` 1000×700·920×700·1280×700 캡처 `target/visual-validation/captures/vde014-panel-1000-112612-202626.png`, `vde014-panel-920-112626-202640.png`, `vde014-panel-1280-112626-202641.png`; 시각 세션 정리 후 프로세스 0·세션 루트 삭제; 실제 VDI 행·숨김 파일·더블클릭·다중 선택 E2E는 VDE-018~019 후속 |
| VDE-015 | GPUI | [x] | [x] | [ ] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (125 passed, 4 ignored); 새 `virtual_disk_copy` 백그라운드 이벤트·대상 입력·진행/중지/완료 카드 GPUI 경계 테스트; 릴리즈 `CLAUDE_LOCAL` 1000×700·920×700·1280×700 캡처 `target/visual-validation/captures/vde015-copy-card-1000-113840-203852.png`, `vde015-copy-card-920-113852-203905.png`, `vde015-copy-card-1280-113852-203905.png`; 시각 세션 정리 후 프로세스 0·세션 루트 삭제; 실제 VDI 복사 성공·중지 E2E는 VDE-018~019 후속 |
| VDE-016 | GPUI | [x] | [x] | [x] | [x] | [x] | [ ] | `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (128 passed, 4 ignored); `virtual_disk_panel_dispatches_explorer_shortcuts_when_directory_is_focused`와 선택 집합·단일 폴더 진입 단위 테스트; 릴리즈 `CLAUDE_LOCAL` 920×700·1000×700·1280×700 캡처 `target/visual-validation/captures/vde016-fixed-920-205304.png`, `vde016-fixed-1000-205250.png`, `vde016-fixed-1280-205319.png`; 실제 표준 VDI `-SelectAllVdi` 시드 캡처 `target/visual-validation/captures/vde019-vbox-select-all-seed-025058.png`에서 16개 선택 확인; `Key Ctrl+A`는 기본 포커스 전환·UI Automation `SetFocus`·`SetWindowPos` 시도 후에도 foreground 안전 차단으로 입력을 보내지 않아 외부 키보드 E2E와 독립 Visual Reviewer는 잔여 |
| VDE-017 | GPUI | [x] | [x] | [x] | [x] | [x] | [ ] | `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (131 passed, 4 ignored); 실행 중 VM 오류·미지원 파일시스템 메시지 단위 테스트와 `virtual_disk_panel_explains_unsupported_partition_state` GPUI 렌더 테스트; 릴리즈 `CLAUDE_LOCAL` 920×700·1000×700·1280×700 캡처 `target/visual-validation/captures/vde017-920-210410.png`, `vde017-1000-210355.png`, `vde017-1280-210423.png`; 시각 세션 정리 후 프로세스 0·세션 루트 0; 실제 VBoxManage 실행 중 VM 및 실제 미지원 파티션 E2E는 VDE-018~019 후속 |
| VDE-019 | E2E | [x] | [x] | [x] | [ ] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 전체 `cargo test --all-targets --all-features --locked` 155 passed·4 ignored, VDI 집중 60 passed·2 ignored; 표준 VDI 헤더 오프셋 수정 후 VirtualBox 7.2.14 `convertfromraw` 생성 VDI(`Storage format: VDI`, `State: created`)를 `GPUI_CONVENIENCE_TOOLS_VBOXMANAGE` 환경 변수와 함께 격리 릴리스 앱에 주입; 숨김/시스템 항목 17개·937,234바이트 복사와 `many_subdirs` 손상 사유 확인; 선택 전 `0개 선택` 캡처와 `-SelectAllVdi` 후 `16개 선택` 캡처 `target/visual-validation/captures/vde019-vbox-key-before-024822.png`, `vde019-vbox-select-all-seed-025058.png`; 기본 포커스 전환·UI Automation `SetFocus`·`SetWindowPos` 모두 foreground를 바꾸지 못해 `Key Ctrl+A`는 안전 차단으로 미검증; 세션 종료 후 `PROCESS_COUNT=0`·`SESSION_COUNT=0`·대상 루트 삭제; 독립 Visual Reviewer는 잔여; commits `87937dc`, `bed5914` |
| VDE-021 | DOCS | — | — | — | — | [x] | [x] | `VIRTUAL_DISK_BACKENDS.md`에 Oracle 공식 `guestcontrol` 계약, 오프라인/실행 중 VM 분리, credential 비저장, 취소·시간 제한·출력 제한·심볼릭 링크 경계를 기록; `VBoxManage.exe`는 설치 경로에서 환경 변수 주입으로 오프라인 실행 중 VM 가드에 사용했지만 Guest Additions·테스트 VM·credential 주입이 없어 실제 Guest Control E2E는 미수행; commit `28dc516` push 완료 |
| D-006 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-007 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-008 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| D-014 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-015 | RUST | [x] | [x] | — | — | [x] | [x] | `SymlinkMode::{Skip, Follow, Recreate}`·`SyncJob.symlink_mode` snake_case 직렬화와 구버전 `Skip` 복원 테스트 통과; Windows 심볼릭 링크 fixture에서 `Follow`·`Recreate`를 조용히 Skip하지 않고 명시적 미구현 실패로 기록하는 경계 테스트 통과; `cargo check -p gpui-convenience-tools --locked`; 전체 `cargo test --all-targets --all-features --locked` 154 passed·4 ignored; `Verify-Workspace.ps1` IDE_VERIFIED; Clippy 표준 실행 exit 0(기존 baseline 경고 7건); 전체 `cargo fmt --check`는 기존 baseline 불일치로 별도 보류 |
| D-016 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-020 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `file_sync_renders_recent_history_with_result_counts_and_duration`로 이력 행 selector와 결과 카드 경계를 시드 상태에서 확인; `file_sync_sections_share_one_width_at_every_window_width`에 이력 카드 폭 회귀 포함; `Verify-Workspace.ps1` 필수 GPUI 18개 및 전체 145 passed·4 ignored; Clippy exit 0(기존 경고 7건); `-SeedHistory`·`-InitialPanel FileSync` 격리 release 캡처 `target/visual-validation/captures/d020-history-panel-012458.png`에서 중지/실패/성공 3행과 건수·소요 시간 확인, processCount=0·sessionCount=0; 독립 Visual Reviewer는 후속; commits `031148e`, `ff9dc72` push 완료 |
| E-001 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-002 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-003 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-004 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| G-001 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `ButtonStyle` 기본값 통일 및 개별 덮어쓰기 제거; 검증 전용 `-InitialPanel`로 패널 전환 입력을 격리 실행에 주입; `cargo check -p gpui-convenience-tools --locked`; 관련 GPUI 테스트 `ad_block_cards_contain_long_content_at_supported_widths`, `service_rows_keep_names_readable_at_supported_window_widths`, `file_sync_sections_share_one_width_at_every_window_width`; 전체 테스트 145 passed·4 ignored; Clippy exit 0(기존 경고 7건); 격리 release 파일 동기화 캡처 `target/visual-validation/captures/g001-file-sync-013019.png`와 자동 시작 캡처 `target/visual-validation/captures/g001-auto-start-013004.png`에서 두 화면과 공용 버튼 스타일 확인, processCount=0·sessionCount=0; 독립 Visual Reviewer는 후속; commits `0d7bf25` 및 후속 검증 경로 커밋 push 완료 |
| K-001 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| G-003 | E2E | [x] | [x] | [x] | [ ] | [x] | [x] | `sidebar_width` 설정 필드와 200~360px 보정·기본 240px 복원; `sidebar_width_is_normalized_to_supported_range`, `update_config_preserves_unedited_fields`, GPUI `sidebar_divider_drag_resizes_navigation_and_content`; 전체 140 passed·4 ignored; Clippy 기존 경고 7건; 격리 release 기본 캡처 `target/visual-validation/captures/g003-default-003709.png`, `sidebar_width=320` 시드 복원 캡처 `target/visual-validation/captures/g003-restored-width-003742.png`; 하네스가 divider 드래그를 지원하지 않아 실제 mouse-up 저장 콜백·독립 Visual Reviewer는 후속; commit `0a793b8` push 완료 |
| G-004 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |

## 완료 검증 기록

완료 작업은 `TODO.md`에서 제거하고 이곳에 게이트 결과를 보존한다. 상세 설계와 완료 단계는
`MASTER_PLAN.md`에 한 번만 기록한다.

| ID | 프로필 | B | T | E | V | R | C | 증거 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AD-001 | RUST | [x] | [x] | — | — | [x] | [x] | `platform::windows::window_ops::tests` 3개 통과; `cargo check --locked` 통과; 최상위 소유/도구 팝업만 후보화, 상태 저장·0×0 동작은 후속 AD-002·AD-003 |
| AD-002 | RUST | [x] | [x] | — | — | [x] | [x] | `window_ops::tests` 5개 통과; `cargo check --locked`; `Verify-AdWindowState.ps1 -ProcessId 26440`로 KakaoTalk/WebView2 관련 PID·창 상태를 읽기 전용 확인; 실제 사용자 창 조작은 수행하지 않음 |
| AD-003 | RUST | [x] | [x] | — | — | [x] | [x] | `window_ops::tests` 7개 통과(전용 0×0 fixture 포함); `cargo check --locked`; `SetWindowPos` 0×0·`EnableWindow(FALSE)`·원상 복원 검증; 사용자 PID 26440은 읽기 전용 상태 확인만 수행 |
| AD-004 | RUST | [x] | [x] | — | — | [x] | [x] | `window_ops::tests` 9개 통과(다중 tool-window fixture·무효 HWND 경계 포함); 다중 HWND 추적·닫힌/재생성 창 정리·프로세스 ID 재사용 방어·수동 상태 변경 재축소·창별 복원 실패 로그 구현; 전체 테스트 및 `Verify-Workspace.ps1` 통과 |
| AD-005 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | ToolHelp 프로세스 트리·`Chrome_WidgetWin_1` 자식 후보 탐색 구현; 사용자 제공 실제 KakaoTalk 광고영역 0×0 확인으로 닫음; 구현 세션의 격리 릴리즈 캡처는 `target/visual-validation/captures/ad-panel-contained-rows-final-2-154739.png`; 커밋 `a9f99d7` 푸시 완료 |
| UI-001 | GPUI | [x] | [x] | [x] | [x] | [x] | [x] | `ad_block_cards_contain_long_content_at_supported_widths`가 920/994/1000/1280px에서 카드·행·클래스 열 경계 통과; 릴리즈 `CLAUDE_LOCAL` 1085×804 캡처 `target/visual-validation/captures/ad-panel-contained-rows-final-2-154739.png`에서 `KakaoTalk.exe`·`Chrome_WidgetWin_1` 표시와 카드 경계 확인; 커밋 `a9f99d7` 푸시 완료 |
| VDE-003 | RUST | [x] | [x] | — | — | [x] | [x] | `virtual_disk` 5개 테스트, 전체 대상 59개 통과; `cargo check --locked` 통과; VDI 파서·UI는 후속 ID 범위 |
| D-001 | RUST | [x] | [x] | — | — | [x] | [x] | `config::tests` 6개 통과(구버전 기본값·새 작업 기본값·직렬화 왕복 포함); `cargo check -p gpui-convenience-tools --locked` 통과; 설정 입력 UI는 D-004 범위 |
| D-002 | RUST | [x] | [x] | — | — | [x] | [x] | `sync::tests` glob 전용 3개 통과; `*`·`?`는 단일 경로 조각, 독립 `**`는 0개 이상 하위 폴더, `/`·`\\` 구분자 정규화 확인; `cargo test --all-targets --all-features --locked` 통과 |
| D-003 | RUST | [x] | [x] | — | — | [x] | [x] | `excluded_files_are_not_copied_and_count_as_skipped`, `excluded_directories_are_not_created_or_removed_by_mirror_deletes` 통과; 상대 경로 glob 적용, `skipped` 계상, 제외 디렉터리의 하위 순회·미러 삭제 보호 확인; 전체 테스트 통과 |
| D-005 | RUST | [x] | [x] | — | — | [x] | [x] | 제외 파일 2개가 대상에 복사되지 않고 `skipped=2`가 되는 회귀 테스트와 제외 디렉터리 보호 테스트 통과; `cargo test --all-targets --all-features --locked` 통과 |
| D-004 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `file_sync_exclude_patterns_editor_is_multiline_and_contained`로 920/994/1280px 입력 영역의 멀티라인 높이·설정 카드 내부 경계 확인; `file_sync_run_button_saves_current_inputs_and_queues_selected_job`로 공백·빈 줄 정리와 줄바꿈 입력의 저장·실행 연결 확인; 격리 릴리즈 캡처는 `target/visual-validation/captures/d4-exclude-patterns-994-editor-visible-170031.png`, `d4-exclude-patterns-1280-editor-visible-170018.png`; 커밋·푸시 완료 |
| VDE-020 | DOCS | — | — | — | — | [x] | [x] | `PROJECT_MAP.md`의 50개 Rust 파일·19,486줄·모듈별 책임/줄 수와 `MASTER_PLAN.md` Phase O-18 이력을 실측 갱신; `git diff --check` 및 활성 ID 정합성 assertion 통과; Phase O 전체 완료 표시는 VDE-019 후속; commit `840a35e` push 완료 |
| T-001 | RUST | [x] | [x] | — | — | [x] | [x] | `overwrites_an_existing_readonly_target_file`로 읽기 전용 대상 파일 속성 해제·덮어쓰기·최종 내용 확인; `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (139 passed, 4 ignored); Clippy exit 0, 기존 경고 7건; commit `8d14458` push 완료 |
| T-002 | RUST | [x] | [x] | — | — | [x] | [x] | `update_config_preserves_unedited_fields`로 서비스·타겟·즐겨찾기·동기화 진행 정보·로그 설정 보존과 디스크 재로드 확인; 임시 `GPUI_CONVENIENCE_TOOLS_DATA_DIR` 격리; `cargo check --locked`; 전체 139 passed·4 ignored; Clippy 기존 경고 7건; commit `8d14458` push 완료 |
| G-002 | RUST | [x] | [x] | — | — | [x] | [x] | `AppRoot` 단일 GPUI 엔티티를 유지하면서 `ServiceState`·`SyncState`·`AdBlockState`로 기능별 상태 소유권 분리; `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` 전체 139 passed·4 ignored; Clippy 기존 경고 7건; 실제 UI 동작 변경이 없는 구조 리팩터링으로 E/V는 N/A; commit `3185369` push 완료 |
| VDE-004 | RUST | [x] | [x] | — | — | [x] | [x] | `virtual_disk::vdi` 4개 테스트 통과; VDI 1.1 헤더·블록 맵·동적 미할당/고정 이미지 읽기·차등 이미지 거부·중복/범위/오버플로 검증; `cargo test --all-targets --all-features --locked`, `cargo check -p gpui-convenience-tools --locked`, `git diff --check` 통과 |
| VDE-005 | RUST | [x] | [x] | — | — | [x] | [x] | `virtual_disk::vdi` 9개 테스트 통과; `.lck` 표식 거부·환경변수 기반 VBoxManage 경로 대조 함수·read-only 핸들·크기/mtime 변경 감지; `cargo test -p gpui-convenience-tools virtual_disk::vdi --locked`, 전체 `cargo test --all-targets --all-features --locked`, `cargo check -p gpui-convenience-tools --locked`, `git diff --check` 통과 |
| VDE-006 | RUST | [x] | [x] | — | — | [x] | [x] | `virtual_disk::partition` 6개 테스트 통과; MBR 기본·확장 EBR 논리 파티션·protective MBR/GPT·주·백업 GPT 헤더 CRC·MBR 범위 초과 검증; `cargo test -p gpui-convenience-tools --all-targets --all-features --locked`, `cargo check -p gpui-convenience-tools --locked`, `cargo build -p gpui-convenience-tools --locked`, `git diff --check` 통과 |
| VDE-007 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools`; `cargo test -p gpui-convenience-tools` (95 passed, 3 ignored); env 주입 실제 NTFS 이미지 테스트 1 passed; commit·push 완료 |
| VDE-008 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (99 passed, 4 ignored); NTFS 오류 경계 7 passed; 실제 NTFS 이미지 1 passed; 손상 부트 섹터 이미지 1 passed; commit·push 완료 |
| VDE-009 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test -p gpui-convenience-tools virtual_disk::copy --all-targets --all-features` (6 passed); `cargo test --all-targets --all-features` (105 passed, 4 ignored); 경로 정책 오류 리뷰 완료; commit·push 완료 |
| VDE-010 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test -p gpui-convenience-tools virtual_disk::copy --all-targets --all-features` (7 passed) 및 `virtual_disk::path_policy` (6 passed); `cargo test --all-targets --all-features` (112 passed, 4 ignored); 청크/충돌 오류 리뷰 완료; commit·push 완료 |
| VDE-011 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 메타데이터 적용·실패 기록 4개 및 NTFS 시간 변환 1개 단위 테스트; `cargo test --all-targets --all-features` (117 passed, 4 ignored); commit·push 완료 |
| VDE-018 | RUST | [x] | [x] | — | — | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test -p gpui-convenience-tools virtual_disk:: --all-targets --all-features` (56 passed, 2 ignored); `cargo test --all-targets --all-features` (135 passed, 4 ignored); `cargo clippy --all-targets --all-features` exit 0 (기존 경고 7건); `app/testdata/ntfs-testfs1.img` 기반 합성 MBR+동적 VDI에서 NTFS 루트·파일 읽기와 원본 바이트 불변성 검증, 고정 이미지 read-only 및 복사본 손상 테스트; rustfmt 대상 파일·`git diff --check`; commit·push 완료 |
| D-017 | RUST | [x] | [x] | — | — | [x] | [x] | `skips_symbolic_links_without_reporting_a_sync_failure`가 실제 Windows 심볼릭 링크를 복사하지 않고 `skipped=1`·실패 목록 비어 있음·일반 파일 복사 성공을 확인; `cargo check -p gpui-convenience-tools --locked`; 전체 141 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `6cf305c` push 완료 |
| D-018 | RUST | [x] | [x] | — | — | [x] | [x] | `sync_history::tests` 2개로 실행 순서 append·손상 JSON 보존을 검증; 동기화 백그라운드 완료 경로에서 작업 ID·라벨·시각·복사/건너뜀/삭제/실패·중지·요약을 `sync-history.json`에 기록; `cargo check -p gpui-convenience-tools --locked`; 전체 143 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `97009c3` push 완료 |
| D-019 | RUST | [x] | [x] | — | — | [x] | [x] | `sync_history::tests::applies_log_count_and_age_retention_before_appending`로 로그 설정의 개수·기간 기준을 새 이력 추가 전에 적용하고 최소 1개를 유지하는 경계 검증; `cargo check -p gpui-convenience-tools --locked`; 전체 144 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `ed71af4` push 완료 |
| D-009 | RUST | [x] | [x] | — | — | [x] | [x] | workspace와 `app`에 `notify 7` 직접 의존성을 선언하고 `Cargo.lock` 애플리케이션 의존성 목록을 갱신; `cargo check -p gpui-convenience-tools --locked`; `cargo test -p gpui-convenience-tools --all-targets --all-features` 전체 145 passed·4 ignored; `cargo tree -e normal -p gpui-convenience-tools`에서 앱 직접 `notify v7.0.0` 확인; Clippy exit 0(기존 경고 7건); `git diff --check` 통과; 실시간 감시 동작은 D-010~D-013 범위로 유지; commit `0315bbd` push 완료 |
| D-010 | RUST | [x] | [x] | — | — | [x] | [x] | `SyncJob`에 `WatchMode::{Interval, Realtime}`와 snake_case 직렬화를 추가하고 `watch_mode` 누락 구버전 설정을 `Interval`로 복원; `sync_job_watch_mode_round_trips_with_interval_compatibility`, `sync_job_without_id_gets_one`; `cargo check -p gpui-convenience-tools --locked`; 전체 146 passed·4 ignored; Clippy exit 0(기존 경고 7건); E/V는 스키마 전용 작업으로 적용하지 않음; 실제 감시·디바운스·설정 UI는 D-011~D-013에서 검증; commit `15e9765` push 완료 |
| D-011 | RUST | [x] | [x] | — | — | [x] | [x] | `app/src/app/watch.rs`에 `notify` 재귀 감시·작업별 2초 quiet debounce·변경 이벤트 기반 실행 예약을 구현; `debounce_coalesces_changes_until_two_seconds_are_quiet`, `realtime_watcher_reports_a_changed_file_after_polling`; 실제 임시 폴더 파일 변경 통합 테스트 포함; `cargo check -p gpui-convenience-tools --locked`; 전체 151 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `e68a3f6` push 완료 |
| D-012 | RUST | [x] | [x] | — | — | [x] | [x] | watcher 생성·경로 감시 실패를 작업별 한 번만 보고하고 `SyncWatchFallback` 이벤트로 주기 모드 강등·로그·경고 토스트를 처리; `watch_failure_is_reported_once_until_the_job_mode_changes`, `file_sync_watch_fallback_downgrades_and_logs`; 전체 151 passed·4 ignored; Clippy exit 0(기존 경고 7건); commit `e68a3f6` push 완료 |
| D-013 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 파일 동기화 설정에 `실시간 감시` 토글을 추가하고 실시간 모드에서는 주기 선택을 숨김; `file_sync_watch_mode_toggle_updates_selected_job`, `Verify-Workspace.ps1` 필수 GPUI 20개; 릴리스 `CLAUDE_LOCAL` 캡처 `target/visual-validation/captures/d011-realtime-panel-015924.png`에서 작업 카드의 `실시간` 상태·토글·2초 안내 확인; 종료 후 processCount=0·sessionCount=0; commit `e68a3f6` push 완료 |

### 합성 VDI 릴리스 E2E

실제 사용자 VDI와 사용자 데이터는 검증에 사용하지 않는다. 먼저 환경 변수로 합성 VDI를
export하고, 하네스가 세션 전용 `source\seed.vdi`와 `target`을 만든 뒤 실제 release 앱에
주입한다.

```powershell
$env:GPUI_CONVENIENCE_TOOLS_EXPORT_VDI_FIXTURE = "D:\검증\vde019-synthetic-ntfs.vdi"
cargo test -p gpui-convenience-tools virtual_disk::vdi::tests::exports_ntfs_vdi_fixture_when_requested -- --nocapture

scripts\Invoke-ClaudeVisualCheck.ps1 -Action Start `
  -SeedVdi target\visual-validation\vde019-synthetic-ntfs.vdi `
  -AutoCopyVdi -InitialPanel VirtualDisk -Width 1200 -Height 1000
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name vde019-copy-summary
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
```

`-AutoCopyVdi`는 검증 전용 환경 변수로만 선택 항목을 임시 대상에 복사한다. 캡처 후에는
`processCount=0`, `sessionCount=0`, 임시 대상 루트 삭제를 확인한다. 이 경로는 합성 VDI와
실제 release 앱의 통합 증거이며, VirtualBox가 직접 생성한 사용자 VDI와 독립 Visual Reviewer
검토를 대신하지 않는다.

### 증거 기록 형식

증거 칸에는 긴 로그를 복사하지 말고 재현 가능한 식별자만 적는다.

```text
명령: cargo test -p gpui-convenience-tools <test_name> -- --nocapture
결과: PASS
수용 기준: <무엇을 확인했는가>
관찰: <실제 결과>
E2E/시각 증거: <PNG 경로 또는 N/A 사유>
오류 리뷰: <없음 또는 오류 ID/원인>
커밋: <commit hash>, origin/main 푸시 확인
```

작업을 완료 처리하면서 행을 삭제하지 않는다. 완료 행은 `MASTER_PLAN.md`의 완료 기록과
커밋을 연결하는 검증 이력으로 남긴다. 단, `TODO.md`에서 완료 작업을 삭제하는 기존 규칙은
그대로 유지한다.
