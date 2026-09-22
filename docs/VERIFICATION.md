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

텍스트 입력처럼 로컬 창 하네스가 지원하지 않는 입력은 GPUI 테스트로 검증하고,
하네스 미지원 상태를 `N/A` 사유 없이 `PASS`로 기록하지 않는다.

최신 품질 게이트 기준은 `cargo check -p gpui-convenience-tools --locked`, 전체 테스트
173 passed·5 ignored, 표준 `cargo clippy -p gpui-convenience-tools --all-targets --all-features --locked -- -D warnings` exit 0·경고
0건이다. `scripts/Verify-Workspace.ps1`도 IDE 안전 모드에서 필수 GPUI 테스트 31개와 전체
테스트를 재실행해 `IDE_VERIFIED`를 출력했다. 실제 데스크톱 표면은 별도 포그라운드 조건이
필요하므로 같은 실행에서 `DESKTOP_PENDING`으로 분리했다. 전체 `cargo fmt --check`는 기존
baseline 포맷 차이가 남아 있어 별도 구조 정리 범위로 유지한다.
이번 실행에서는 `DOCS_VERIFIED active=26 matrix=26`도 먼저 통과해 활성 TODO와 검증
매트릭스의 일대일 대응을 확인했다.

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
이 스크립트는 코드 테스트 전에 `scripts/Assert-ProjectDocs.ps1`를 실행해 활성 TODO ID와
검증 매트릭스의 일대일 대응도 확인한다.

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
# 필요한 경우 Click·Wheel·Drag 실행 후 반드시 다시 Capture
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
| VDE-024 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 지연 로딩 `GuestDirectoryTreeNode`와 좌우 `balanced_split` 탐색 영역을 추가해 폴더 트리 선택으로 중첩 경로를 바로 열고, 목록 행 높이·열 폭·탐색 카드 높이를 컴팩트하게 조정; `virtual_disk_folder_tree_navigates_nested_paths_without_repeated_list_clicks`, `virtual_disk_explorer_keeps_tree_and_file_list_inside_compact_card`와 전체 169 passed·4 ignored, Clippy `-D warnings` 통과; `Verify-Workspace.ps1` 필수 GPUI 테스트 29개 이름 검증; release 캡처 `target/visual-validation/captures/vde024-tree-920x700-121233.png`, `vde024-tree-1000x700-121200.png`, `vde024-tree-1280x900-121234.png`; 실제 트리 Click은 하네스 좌표 변환으로 상태 전환을 확정하지 않아 GPUI 이벤트 테스트로 대체, commit `4af7cc6` push 완료; 독립 Visual Reviewer는 후속 |
| VDE-025 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `VirtualDiskLayoutConfig`에 트리·종류·속성·크기 폭과 기본값/최소·최대 보정을 추가하고 `settings.json` round-trip을 검증; 트리 스플리터는 150~240px, 파일명은 남은 폭, 속성·크기 열은 최소 읽기 폭을 사용하며 속성·크기 조절 및 기본값 복원 UI를 추가; `virtual_disk_layout_controls_update_and_reset_readable_column_widths`, `virtual_disk_tree_width_setting_is_clamped_and_persisted`, 기존 트리·카드 bounds 테스트와 전체 172 passed·4 ignored, Clippy `-D warnings`, 필수 GPUI 31개 통과; release 캡처 `target/visual-validation/captures/vde025-final-920x700-scrolled-124559.png`, `vde025-final-1000x700-124600.png`, `vde025-final-capped-tree-1280x900-124829.png`; 실제 스플리터 Drag는 포그라운드 안전 경계로 미수행, commit `195a43e` push 완료; 독립 Visual Reviewer는 후속 |
| VDE-022 | RUST | [x] | [x] | [x] | [ ] | [x] | [x] | 실제 `D:\VMMachine\win11\TACS\TACS_1.vdi`를 읽기 전용으로 대조해 `VDI normal (base)`, `dynamic default`, GPT 1번 MSR, 2번 `-FVE-FS-` BitLocker를 확인; `classifies_bitlocker_and_microsoft_reserved_partitions_explicitly`, `encrypted_partition_error_explains_the_offline_boundary`, 전체 테스트 163 passed·4 ignored, release 캡처 `target/visual-validation/captures/vde022-final-095347.png`에서 MSR·BitLocker 라벨과 안내를 확인; commit `6b89a09` push 완료; 독립 Visual Reviewer는 후속 |
| VDE-023 | GPUI | [x] | [x] | [ ] | [x] | [x] | [x] | `gpui::PathPromptOptions { files: true, directories: false, multiple: false }` 기반 네이티브 파일 선택 버튼과 경로 입력 반영 구현, `virtual_disk_panel_registers_navigation_and_renders_read_only_shell`에서 `virtual-disk-browse` bounds 확인, release 캡처 `target/visual-validation/captures/vde022-final-095347.png`에서 `찾아보기` 배치 확인; 실제 대화상자 열기·선택 입력은 포그라운드 안전 경계로 미수행; commit `6b89a09` push 완료 |
| VDE-012 | E2E | [x] | [x] | [x] | [ ] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 160 passed·4 ignored; `virtual_disk::copy::tests::collecting_copy_errors_removes_partial_output_and_keeps_issue`로 원본 단축 읽기 실패 뒤 부분 대상 파일 제거와 원본 `SourceChanged` 사유 보존을 확인하고, 오류 행 selector·억제 키 저장/재로드·미억제 토스트 게이트와 `virtual_disk_panel_renders_copy_progress_and_issue_summary`의 GPUI `simulate_click` 억제/재표시 dispatch를 검증; 실제 VBox 생성 VDI 릴리스 복사에서 `many_subdirs` 손상 사유·17개 파일 부분 결과를 확인; 독립 Visual Reviewer와 실제 릴리스 버튼 조작은 잔여 |
| VDE-014 | GPUI | [x] | [x] | [ ] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `cargo test --all-targets --all-features` (161 passed, 4 ignored); `virtual_disk_directory_row_double_click_enters_directory_and_refreshes_entries`가 실제 렌더 행에 `MouseDown/MouseUp click_count=2`를 전달해 폴더 진입·새로고침·오류 목록 제거를 확인하고, `shift_range_selection_uses_anchor_in_both_directions`로 순방향·역방향 Shift 범위 선택과 기준점 소실 경계를 확인했으며 GPUI 셸 테스트에 상위 이동 액션 경계를 포함; `virtual_disk_clears_stale_entries_when_directory_refresh_fails`로 손상 하위 폴더 진입 시 이전 목록 제거 확인; 최종 release 1200×1000 캡처 `target/visual-validation/captures/vde014-initial-guest-path-many-subdirs-final-042633.png`에서 현재 경로·손상 사유·빈 목록 확인; 최신 release 캡처 `target/visual-validation/captures/vde014-shift-range-release-final-045106.png`에서 16개 선택·숨김/시스템 행·카드 경계를 확인; 실제 폴더 진입 Click과 Shift 마우스 입력은 포그라운드 안전 검사에서 차단되어 E2E 성공으로 기록하지 않음; 세션 종료 후 프로세스 0·세션 루트 0 |
| VDE-015 | GPUI | [x] | [x] | [x] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 161 passed·4 ignored; `virtual_disk_copy` 백그라운드 이벤트·대상 입력·진행/중지/완료 카드 GPUI 경계 테스트, `virtual_disk::copy::tests::cancellation_stops_between_chunks_and_removes_partial_file`의 청크 경계 중지·부분 대상 파일 제거, `virtual_disk::copy::tests::collecting_copy_errors_removes_partial_output_and_keeps_issue`의 원본 단축 읽기 실패 정리를 확인; 릴리즈 `CLAUDE_LOCAL` 1000×700·920×700·1280×700 캡처 `target/visual-validation/captures/vde015-copy-card-1000-113840-203852.png`, `vde015-copy-card-920-113852-203905.png`, `vde015-copy-card-1280-113852-203905.png`; 최신 표준 VDI 자동 복사 캡처 `target/visual-validation/captures/vde015-continuation-autocopy-1850-035033.png`에서 파일 17개·915.3KB·실패 1개 요약 확인, 격리 대상 `-Force` 재검사 17개·937,234바이트와 손상 `many_subdirs` 사유 확인; 종료된 TACS VDI의 `$Extend/$RmMetadata/$TxfLog`를 `-ReadDelayMs 1000 -CancelAfterMs 2200`으로 실행해 실제 2MiB 파일 복사 중 0바이트 부분 파일 생성·취소 후 제거와 `중지됨 · 파일 2개 · 64.0 KB`를 확인하고 캡처 `target/visual-validation/captures/vde015-txf-log-slow-chunk-before-cancel-062617.png`, `target/visual-validation/captures/vde015-txf-log-slow-chunk-after-cancel-062619.png`를 남김; 원본 길이 85,269,151,744바이트·VM poweroff·세션 종료 후 process/session 0; 256MB NTFS VHD fixture 생성은 `diskpart` 50초 무응답으로 중지되어 VHD·드라이브·잔류 프로세스가 없음을 확인했으며 자연 속도 대용량 VDI 중지·독립 Visual Reviewer는 후속 |
| VDE-016 | GPUI | [x] | [x] | [x] | [x] | [x] | [ ] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 160 passed·4 ignored; `virtual_disk_panel_dispatches_explorer_shortcuts_when_directory_is_focused`와 선택 집합·단일 폴더 진입 단위 테스트; 릴리즈 `CLAUDE_LOCAL` 920×700·1000×700·1280×700 캡처 `target/visual-validation/captures/vde016-fixed-920-205304.png`, `vde016-fixed-1000-205250.png`, `vde016-fixed-1280-205319.png`; 실제 표준 VDI `-SelectAllVdi` 시드 캡처 `target/visual-validation/captures/vde019-vbox-select-all-seed-025058.png`에서 16개 선택 확인; `Key Ctrl+A`는 기본 포커스 전환·UI Automation `SetFocus`·`SetWindowPos` 시도 후에도 foreground 안전 차단으로 입력을 보내지 않아 외부 키보드 E2E와 독립 Visual Reviewer는 잔여 |
| VDE-017 | GPUI | [x] | [x] | [x] | [x] | [x] | [ ] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 160 passed·4 ignored; 실행 중 VM 오류·미지원 파일시스템 메시지 단위 테스트, `reads_unsupported_partition_fixture_without_claiming_ntfs`, `exports_unsupported_partition_vdi_fixture_when_requested`, `virtual_disk_panel_explains_unsupported_partition_state` GPUI 렌더 테스트; 미지원 MBR 파티션 VDI를 최신 release 앱에 주입해 `vde017-unsupported-partition-release-fixed-033450.png`에서 `지원하지 않는 파일시스템` 오류 카드와 `미지원/미확인 파일시스템` 파티션 경고를 확인; 읽기 전용 `VBoxManage list runningvms`가 비어 있고 TACS `VMState=poweroff`여서 실행 중 VM E2E는 미수행; 세션 정리 후 프로세스 0·세션 루트 0; 독립 Visual Reviewer는 후속 |
| VDE-019 | E2E | [x] | [x] | [x] | [ ] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 160 passed·4 ignored, VDI 집중 60 passed·2 ignored; 표준 VDI 헤더 오프셋 수정 후 VirtualBox 7.2.14 `convertfromraw` 생성 VDI(`Storage format: VDI`, `State: created`)를 `GPUI_CONVENIENCE_TOOLS_VBOXMANAGE` 환경 변수와 함께 격리 릴리스 앱에 주입; 숨김/시스템 항목 17개·937,234바이트 복사와 `many_subdirs` 손상 사유 확인; 최신 release 1200×1000 캡처 `target/visual-validation/captures/continuation-vde019-current-release-040648.png`에서 숨김/시스템 행과 17개·915.3KB·실패 1건 요약을 재확인; 이번 구현 세션에서도 1200×1000 `target/visual-validation/captures/continuation-current-vde019-1200x1000-052717.png`와 920×700 `target/visual-validation/captures/continuation-current-vde019-920x700-052734.png`를 캡처해 안전 경계·NTFS 3.1·숨김/시스템 행·16개 선택과 compact 폭 경계를 확인; 선택 전 `0개 선택` 캡처와 `-SelectAllVdi` 후 `16개 선택` 캡처 `target/visual-validation/captures/vde019-vbox-key-before-024822.png`, `vde019-vbox-select-all-seed-025058.png`; 기본 포커스 전환·UI Automation `SetFocus`·`SetWindowPos` 모두 foreground를 바꾸지 못해 `Key Ctrl+A`는 안전 차단으로 미검증; 세션 종료 후 `PROCESS_COUNT=0`·`SESSION_COUNT=0`·대상 루트 삭제; 이번 캡처는 구현 세션 증거이며 외부 키보드·실제 릴리스 오류 억제 버튼 클릭·독립 Visual Reviewer는 잔여; commits `87937dc`, `bed5914` |
| VDE-021 | DOCS | — | — | — | — | [x] | [x] | `VIRTUAL_DISK_BACKENDS.md`에 Oracle 공식 `guestcontrol` 계약, 오프라인/실행 중 VM 분리, credential 비저장, 취소·시간 제한·출력 제한·심볼릭 링크 경계를 기록; 읽기 전용 `VBoxManage list runningvms`는 비어 있고 TACS `VMState=poweroff`이며 Guest Additions·credential 주입 근거가 없어 실제 Guest Control E2E는 미수행; commit `28dc516` push 완료 |
| D-014 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-015 | RUST | [x] | [x] | — | — | [x] | [x] | `SymlinkMode::{Skip, Follow, Recreate}`·`SyncJob.symlink_mode` snake_case 직렬화와 구버전 `Skip` 복원 테스트 통과; Windows 심볼릭 링크 fixture에서 `Follow`·`Recreate`를 조용히 Skip하지 않고 명시적 미구현 실패로 기록하는 경계 테스트 통과; `cargo check -p gpui-convenience-tools --locked`; 전체 `cargo test --all-targets --all-features --locked` 154 passed·4 ignored; `Verify-Workspace.ps1` IDE_VERIFIED; Clippy 표준 실행 exit 0(기존 baseline 경고 7건); 전체 `cargo fmt --check`는 기존 baseline 불일치로 별도 보류 |
| D-016 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-020 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; `file_sync_renders_recent_history_with_result_counts_and_duration`로 이력 행 selector와 결과 카드 경계를 시드 상태에서 확인; `file_sync_sections_share_one_width_at_every_window_width`에 이력 카드 폭 회귀 포함; `Verify-Workspace.ps1` 필수 GPUI 18개 및 전체 145 passed·4 ignored; Clippy exit 0(기존 경고 7건); `-SeedHistory`·`-InitialPanel FileSync` 격리 release 캡처 `target/visual-validation/captures/d020-history-panel-012458.png`에서 중지/실패/성공 3행과 건수·소요 시간 확인; 최신 release 994×702·1280×900 캡처 `next-d020-history-065416.png`·`next-d020-history-1280-065430.png`에서 동일 경계를 재확인하고 1280×900에서 3행 전체 표시, processCount=0·sessionCount=0; 독립 Visual Reviewer는 후속; commits `031148e`, `ff9dc72` push 완료 |
| E-001 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-002 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-003 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-004 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| G-001 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `ButtonStyle` 기본값 통일 및 개별 덮어쓰기 제거; 검증 전용 `-InitialPanel`로 패널 전환 입력을 격리 실행에 주입; `cargo check -p gpui-convenience-tools --locked`; 관련 GPUI 테스트 `ad_block_cards_contain_long_content_at_supported_widths`, `service_rows_keep_names_readable_at_supported_window_widths`, `file_sync_sections_share_one_width_at_every_window_width`; 전체 테스트 145 passed·4 ignored; Clippy exit 0(기존 경고 7건); 격리 release 파일 동기화 캡처 `target/visual-validation/captures/g001-file-sync-013019.png`와 자동 시작 캡처 `target/visual-validation/captures/g001-auto-start-013004.png`에서 두 화면과 공용 버튼 스타일 확인, processCount=0·sessionCount=0; 독립 Visual Reviewer는 후속; commits `0d7bf25` 및 후속 검증 경로 커밋 push 완료 |
| K-001 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| G-003 | E2E | [x] | [x] | [x] | [ ] | [x] | [x] | `sidebar_width` 설정 필드와 200~360px 보정·기본 240px 복원; `sidebar_width_is_normalized_to_supported_range`, `update_config_preserves_unedited_fields`, GPUI `sidebar_divider_drag_resizes_navigation_and_content`; 전체 최신 테스트는 상단 품질 게이트 기준 161 passed·4 ignored; 격리 release 기본 캡처 `target/visual-validation/captures/g003-default-003709.png`, `sidebar_width=320` 시드 복원 캡처 `target/visual-validation/captures/g003-restored-width-003742.png`; 최신 release 시작 캡처 `target/visual-validation/captures/next-g003-before-065645.png`에서 divider 위치를 재확인했으나 `-Action Drag -X 0.25 -Y 0.50 -ToX 0.32 -ToY 0.50`은 포그라운드 안전 검사(`현재 포그라운드=66246`)에서 차단되어 입력을 보내지 않음; 실제 mouse-up 저장 콜백과 독립 Visual Reviewer는 후속 |
| K-002 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | macOS 메뉴 막대 지원은 macOS 실행 환경 확인 후 진행 |
| K-003 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | `launchd` 자동 시작은 macOS 실행 환경 확인 후 진행 |
| K-004 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | `.icns` 배포 아이콘은 macOS 패키징 환경 확인 후 진행 |
| K-005 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | 코드 서명·공증은 Apple Developer 계정 확보 후 진행 |

최신 VDE-019 릴리스 보강 검증은 `exports_ntfs_vdi_fixture_when_requested`로 격리 합성 VDI를
재생성한 뒤 수행했다. 최신 release 앱에서 표준 VBox VDI와 재생성 합성 VDI 모두 파일 행,
숨김·시스템 속성, 복사 결과 17개·915.3KB, `many_subdirs` 실패 사유를 확인했고 캡처는
`continuation-vde019-standard-release-031918.png`,
`continuation-vde019-synthetic-refreshed-032017.png`이다. 오류 억제 버튼 `Click`은
포그라운드 안전 검사에서 차단되어 실제 릴리스 클릭 성공으로 기록하지 않았으며, 세션 종료 후
`PROCESS_COUNT=0`·`SESSION_COUNT=0`을 확인했다.

VDE-017 미지원 파티션 릴리스 검증은 `exports_unsupported_partition_vdi_fixture_when_requested`로
MBR 타입 `0x83`·NTFS 부트 시그니처가 없는 VDI를 격리 생성한 뒤 수행했다. 최신 release 앱에서
파티션 행의 `미지원/미확인 파일시스템` 상태와 `지원하지 않는 파일시스템` 오류 카드를 함께
확인했으며 캡처는 `vde017-unsupported-partition-release-fixed-033450.png`이다. 검증 후
`PROCESS_COUNT=0`·`SESSION_COUNT=0`을 확인했다. fixture가 NTFS로 오인되지 않는지
`reads_unsupported_partition_fixture_without_claiming_ntfs` 단위 테스트도 통과했다.

G-003의 실제 divider 드래그 경로는 검증 하네스에 `-Action Drag -X <start> -Y <start>
-ToX <end> -ToY <end>`를 추가한 뒤 1000×700 release 화면에서 시도했다. 시작 캡처는
`target/visual-validation/captures/g003-drag-before-063747.png`에 남겼지만, 현재 데스크톱의
포그라운드를 검증 창으로 전환하지 못해 하네스가 전역 입력을 보내지 않고 중단했다. 따라서
실제 mouse-up 저장 콜백 성공으로 기록하지 않으며, 세션 정리 후 `PROCESS_COUNT=0`·
`SESSION_COUNT=0`을 확인했다. GPUI `sidebar_divider_drag_resizes_navigation_and_content`
회귀 테스트와 하네스 구문 검증은 통과했다.

## 완료 검증 기록

완료 작업은 `TODO.md`에서 제거하고 이곳에 게이트 결과를 보존한다. 상세 설계와 완료 단계는
`MASTER_PLAN.md`에 한 번만 기록한다.

| ID | 프로필 | B | T | E | V | R | C | 증거 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C-001 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 실행파일 옆 `settings.json` 우선 경로·구버전 AppData `config.json` fallback·`GPUI_CONVENIENCE_TOOLS_DATA_DIR` 격리 경계와 `VirtualDiskConfig` 왕복을 구현; 정상 실행 경로가 실행파일 옆 `settings.json`을 가리키는 회귀 테스트와 설정 테스트 전용 환경 잠금 추가; GPUI `settings_page_keeps_user_config_actions_inside_the_card` 통과; 설정 화면에 현재 경로·`설정 저장`·`파일 위치 열기`를 추가했고 창 닫기/Drop 저장 경계를 연결; 전체 167 passed·4 ignored, Clippy `-D warnings` 통과, `DOCS_VERIFIED active=28 matrix=28`, `STRUCTURE_VERIFIED files=56 lines=22003 max=920 path=app/src/config.rs warnings=4`; release 설정 화면 1000×700 `target/visual-validation/captures/c001-settings-1000x700-111818.png`, 920×700 `c001-settings-920x700-111840.png`, VDI 입력 복원 `c001-vdi-restored-1000x700-112034.png`; commit `1d7b972` push 완료 |
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
| D-006 | RUST | [x] | [x] | — | — | [x] | [x] | 3,000개 파일 3회 측정에서 사전 순회 `0.003초`, 실제 동기화 `3.262~3.308초`, 추가 비용 약 `0.1%`; 종료 TACS `TACS.vdi` NTFS 파티션 1의 읽기 전용 트리 1회 `0.524초`·2회 `0.127초`; 사전 순회 채택 결정과 수동 측정 테스트 고정; `git diff --check` 통과 |
| D-007 | RUST | [x] | [x] | — | — | [x] | [x] | `count_sync_entries`·`SyncProgress.total`·백그라운드 이벤트 전달 구현; `pre_scan_counts_the_same_processable_entries_as_sync_progress` 통과; `cargo check --locked`, 전체 173 passed·5 ignored, Clippy `-D warnings`, 문서·구조 게이트 통과; commit `4e29479` push 완료 |
| D-008 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `file_sync_status_bar_stays_visible_at_compact_height_while_running`에서 920×480 bounds와 진행률 바 포함을 확인; 전체 173 passed·5 ignored, 필수 GPUI 31개, Clippy `-D warnings`; release 실제 동기화 중 캡처 `target/visual-validation/captures/d008-progress-running-1000x700-205653.png`에서 `동기화 중`, `복사 6135 · 건너뜀 4000 · 실패 0`과 진행률 바 확인; 세션 종료 후 process/session 0, commit `4e29479` push 완료 |
| D-004 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `file_sync_exclude_patterns_editor_is_multiline_and_contained`로 920/994/1280px 입력 영역의 멀티라인 높이·설정 카드 내부 경계 확인; `file_sync_run_button_saves_current_inputs_and_queues_selected_job`로 공백·빈 줄 정리와 줄바꿈 입력의 저장·실행 연결 확인; 격리 릴리즈 캡처는 `target/visual-validation/captures/d4-exclude-patterns-994-editor-visible-170031.png`, `d4-exclude-patterns-1280-editor-visible-170018.png`; 커밋·푸시 완료 |
| VDE-020 | DOCS | — | — | — | — | [x] | [x] | `PROJECT_MAP.md`의 50개 Rust 파일·19,486줄·모듈별 책임/줄 수와 `MASTER_PLAN.md` Phase O-18 이력을 실측 갱신; `git diff --check` 및 활성 ID 정합성 assertion 통과; Phase O 전체 완료 표시는 VDE-019 후속; commit `840a35e` push 완료 |
| VDE-013 | GPUI | [x] | [x] | [x] | [x] | [x] | [x] | `virtual_disk_panel_registers_navigation_and_renders_read_only_shell`; release `CLAUDE_LOCAL` 캡처 `vde013-panel-920-final-201106.png`, `vde013-panel-1280-final-201106.png`; 후속 실제 VirtualBox 7.2.14 VDI 캡처 `vde019-vbox-standard-loaded-023912.png`에서 VDI 경로·MBR/NTFS 파티션·게스트 루트 연결을 확인하고 새로고침 경계를 GPUI 테스트로 검증; 세션 종료 후 프로세스·세션 루트 0; commit `83c579b` |
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

대용량 종료 VM 디스크를 검증할 때는 원본을 세션 폴더로 복사하지 않도록
`-ExternalVdiPath <absolute-vdi-path>`를 사용한다. 앱은 이 경로를 `VdiReader`의 read-only
입력으로만 열고, 대상 폴더·설정·로그는 세션 임시 루트에 격리한다. 하네스는 설치된
`VBoxManage.exe`를 환경 변수로 주입해 실행 중 VM 가드를 활성화한다. 이 옵션은 원본 파일을
복사하지 않으므로 실제 사용자 VDI에 사용할 때도 VM이 종료된 상태인지 먼저 확인해야 한다.

2026-09-22 현재 종료된 `TACS` VM의 실제 VDI를 이 경로로 열어 `TACS.vdi`에서 GPT 파티션
1·5의 NTFS 3.1과 게스트 루트의 시스템 폴더를 확인했다. 920×700 캡처
`target/visual-validation/captures/vde019-external-tacs-root-fixed-920x700-053642.png`와
1200×1000 캡처 `target/visual-validation/captures/vde019-external-tacs-root-fixed-1200x1000-053659.png`를
남겼으며, 원본 길이는 변하지 않았고 세션 종료 후 `PROCESS_COUNT=0`·`SESSION_COUNT=0`이었다.
대용량 파일 중지·외부 키보드·독립 Visual Reviewer는 이 검증으로 완료 처리하지 않는다.

복사 중지 경계는 검증 전용 `-CancelAfterMs <milliseconds>`로도 재현할 수 있다. 이 옵션은
`-AutoCopyVdi`와 함께 사용할 때 복사 시작 후 앱의 중지 메서드를 호출하며, 설정 파일이나
일반 사용자 실행 경로에는 저장하지 않는다. 종료된 실제 `TACS.vdi`의 200ms 시나리오에서
release 앱 요약 `중지됨 · 파일 0개 · 0 B · 건너뜀 0 · 실패 0`을 확인했고 캡처
`target/visual-validation/captures/vde015-external-tacs-cancel-1200x1000-054536.png`를
남겼다. 같은 VDI의 1,500ms 실행은 실제 루트 데이터가 먼저 끝나 `완료 · 파일 19개 · 4.1 MB`가
되었고, `$Extend`의 1,000ms 실행도 `완료 · 파일 8개 · 4.1 MB`가 되어 실제 대용량 파일의
청크 중간 중지는 재현되지 않았다. 따라서 이 시드는 중지 상태·정리 경계의 보조 증거일 뿐,
VDE-015의 실제 대용량 파일 중지 완료 증거로 승격하지 않는다.

다중 파티션 VDI를 검증할 때는 `-VdiPartitionNumber <번호>`를 지정한다. 지정하지 않으면
기존 호환 동작대로 첫 번째 파티션을 선택한다. 실제 종료된 TACS VM에서 파티션 5번을
선택한 캡처 `target/visual-validation/captures/vde015-tacs-partition5-root-060904.png`를
확인했고, 파티션 1번 `$Extend` 경로의 `-CancelAfterMs 250`은
`중지됨 · 파일 0개 · 0 B`로 끝났으며 캡처는
`target/visual-validation/captures/vde015-tacs-extend-cancel-061128.png`이다. 같은 경로의
`600`ms 실행은 `완료 · 파일 8개 · 4.1 MB`였고 캡처는
`target/visual-validation/captures/vde015-tacs-extend-cancel-600-061151.png`이다. 두 실행은
원본 VDI를 변경하지 않았지만 실제 대용량 파일의 청크 중간 중지를 증명하지 않으므로 VDE-015
완료 조건으로 승격하지 않는다.

같은 종료된 TACS VDI의 `$Extend/$RmMetadata/$TxfLog`에는 64.0KB 파일과 2MiB 파일 2개가
있어, 자연 속도에서 먼저 완료되는 작은 항목과 큰 항목의 중지 경계를 분리해 확인했다.
검증 전용 `-ReadDelayMs 1000 -CancelAfterMs 2200`을 사용했으며 이 지연은 일반 사용자
설정에 저장되지 않고 복사 작업 스레드의 게스트 `read_at` 호출에만 적용된다. 실제 release
앱 캡처 `target/visual-validation/captures/vde015-txf-log-slow-chunk-before-cancel-062617.png`
와 `target/visual-validation/captures/vde015-txf-log-slow-chunk-after-cancel-062619.png`에서
동일한 4개 선택 항목과 `중지됨 · 파일 2개 · 64.0 KB` 요약을 확인했다. 실행 중 대상에는
`$TxfLogContainer00000000000000000001` 0바이트 파일이 생성된 상태가 관찰되었고, 취소 후
그 미완성 파일은 제거되며 이미 완료된 `$TxfLog.blf`만 남았다. 원본 길이
`85,269,151,744`바이트 불변, VM `poweroff`, 세션 종료 후 `PROCESS_COUNT=0`·
`SESSION_COUNT=0`을 확인했다. 이는 실제 VDI의 파일·복사 엔진·부분 파일 정리 경계를
잇는 보조 E2E 증거이며, 자연 속도의 청크 중지나 독립 Visual Reviewer 확인을 완료 처리하지
않는다.

2026-09-22 2차 자체 시각 교차 확인으로 D-020·G-001의 최신 release 화면을 별도 격리
세션에서 다시 열었다. D-020은 994×702에서 최근 실행 이력 카드가 세로 스크롤 안에
배치되고, 1280×900에서 중지·실패 포함·성공 3행이 모두 카드 경계 안에 표시되었다.
캡처는 `target/visual-validation/captures/d020-second-pass-before-055457.png`와
`target/visual-validation/captures/d020-second-pass-resized-055501.png`이다. G-001은
994×702·1280×900 모두 자동 시작 상태 카드·작업 제어·안내 문장이 폭 밖으로 밀리지 않았다.
캡처는 `target/visual-validation/captures/g001-second-pass-before-055522.png`와
`target/visual-validation/captures/g001-second-pass-resized-055525.png`이다. 이는 같은
에이전트의 별도 세션에서 수행한 2차 자체 확인이며 독립 Visual Reviewer 검토를 대체하지
않는다. 각 세션 종료 후 프로세스와 세션 루트는 0개였다.

폴더 진입 화면을 입력 없이 재현할 때는 `-InitialGuestPath many_subdirs`를 함께 사용한다.
이 값은 검증 프로세스에만 전달되며 제품 설정에는 저장되지 않는다.

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
