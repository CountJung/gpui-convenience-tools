# 작업별 검증 실행·기록

> 이 문서는 코드 수정 결과를 작업 ID별로 검증하고 증거를 남기는 실행 정본이다.
> 공통 판정 기준은 `DEVELOPMENT_GUIDE.md`, GPUI 화면 절차는
> `.github/skills/gpui-visual-check/SKILL.md`를 참조한다. 이 문서에는 실행 순서와
> 작업별 체크 상태만 기록한다.

## 현재 E2E 경로

이 저장소에는 브라우저용 E2E 하네스가 없다. 수용 기준에 맞춰 다음 경로를 선택한다.

1. **GPUI 상호작용 수용 테스트** — `#[gpui::test]`에서 실제 대상 뷰를 렌더링하고
   `TestAppContext`/`VisualTestContext`로 리사이즈·클릭·키보드·wheel을 입력한 뒤 앱 상태,
   선택·포커스, 스크롤 offset, `debug_bounds`를 단언한다. 앱 내부 동작은 이 경로로 충분하다.
2. **데스크톱 black-box 검증** — Windows에서 release 바이너리를 격리 실행하고
   `scripts/Invoke-ClaudeVisualCheck.ps1` 또는 데스크톱 handoff 절차로 실제 창 전환·스크롤·
   캡처를 확인한다. 화면 표시와 OS 연동 경계에 필요한 경우만 추가한다.

하네스가 지원하지 않는 내부 입력은 의미 있는 GPUI 테스트로 검증한다. OS 경계의 자동화가
어려우면 사용자 직접 확인을 기록할 수 있다. 두 담당자의 실제 화면 반복 검증은 기본
완료 조건이 아니다. 적용 수준의 정본은 `DEVELOPMENT_GUIDE.md`다.

최신 품질 게이트 기준은 `cargo check -p gpui-convenience-tools --locked`, 전체 테스트
186 passed·5 ignored, 표준 `cargo clippy -p gpui-convenience-tools --all-targets --all-features --locked -- -D warnings` exit 0·경고
0건이다. `scripts/Verify-Workspace.ps1`도 IDE 안전 모드에서 필수 GPUI 테스트 34개와 전체
테스트를 재실행해 `IDE_VERIFIED`를 출력했다. 실제 데스크톱 표면은 별도 포그라운드 조건이
필요하므로 같은 실행에서 `DESKTOP_PENDING`으로 분리했다. 전체 `cargo fmt --check`는 기존
baseline 포맷 차이가 남아 있어 별도 구조 정리 범위로 유지한다.
이번 실행에서는 `DOCS_VERIFIED active=23 matrix=23`도 먼저 통과해 활성 TODO와 검증
매트릭스의 일대일 대응을 확인했다.

AD-006을 포함한 현재 변경 코드의 검증 결과는 전체 199 passed·7 ignored 및 Clippy `-D warnings`
통과다. 위 `IDE_VERIFIED`와 `DOCS_VERIFIED`는 이전 기준 실행 결과이므로 이 수치의
재실행 증거로 간주하지 않는다.

## 작업마다 적용하는 순서

작업 ID 하나를 완료 처리하기 전에 아래 순서를 따른다. 문서 전용 작업은 해당 없는 코드
게이트를 `N/A`로 기록하고 문서 assertion을 대신 실행한다.

### 1. 범위와 검증 프로필 선택

| 프로필 | 적용 대상 | 필수 검증 |
| --- | --- | --- |
| `RUST` | 순수 Rust·플랫폼·설정·파일 I/O | `cargo check --locked` + 관련 테스트 + 전체 테스트(단계 완료 시) |
| `GPUI` | 레이아웃·테마·포커스·스크롤·사용자 입력 | 관련 이벤트·결과 `#[gpui::test]`; 공용/다중 모듈 변경 때 전체 테스트 |
| `E2E` | 여러 상태/화면을 잇는 사용자 흐름 | `GPUI` 수용 테스트 + 필요한 OS 경계의 대표 실제 확인 |
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

UI를 바꿀 때는 필요한 수용 기준과 기존 테스트를 연결하고 빠진 단언만 보강한다.

- 기본 창 크기와 최소 지원 창 크기를 모두 사용한다.
- 입력 전 상태, 입력(`simulate_click`, `simulate_keystrokes`, `simulate_event`, wheel),
  입력 후 상태와 기대 bounds를 단언한다.
- 안정적인 `debug_selector`를 부여하고 취약한 절대 좌표만으로 통과시키지 않는다.
- 앱이 단순히 panic 없이 렌더링된 것만으로는 통과로 보지 않는다.
- 새 테스트는 `scripts/Verify-Workspace.ps1`의 필수 목록에도 등록한다.

### 4. 실제 데스크톱 E2E/시각 검증

Windows 로컬 하네스 경로에서는 다음처럼 작업 전용 세션을 사용한다. 상세 좌표·시드·정리 규칙은
`.github/skills/gpui-visual-check/SKILL.md`만 참조한다.

```powershell
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
cargo build -p gpui-convenience-tools --release
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Start -Width 994 -Height 702
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name before
# 필요한 경우 Click·Wheel·Drag 실행 후 반드시 다시 Capture
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
```

`Capture`만으로 충분하면 전역 입력을 보내지 않는다. 입력이 필요했다면 각 동작의
`restore.cursor`·`restore.focus`와 `Start`/`Stop`의 포커스 복원 결과를 확인한다.
복원 실패는 그대로 기록한다. `skipped-focus-changed`만으로 입력 불가능이나 사용자 작업 중을
판단하지 않는다. 화면 점유 가능성은 입력 전에 알린다.

VS Code/Codex의 `IDE` 표면에서는 화면 조작을 시도하지 않고 다음으로 handoff를 준비한다.

```powershell
pwsh -NoProfile -File .\scripts\Verify-Workspace.ps1 -PrepareDesktopHandoff
```

`CLAUDE_LOCAL`/`DESKTOP` 검증은 캡처 PNG, 창 크기, 시나리오, 기대·관찰 결과, 바이너리 커밋과
SHA-256을 기록한다. Visual Reviewer는 필요한 항목만 선택 검토하며 새 실제 재현과 기존
증거 검토를 구분한다. 자동 테스트가 다루는 내부 조작은 데스크톱에서 반복하지 않아도 된다.

### 5. 완료 체크와 커밋

작업 ID의 이 문서 행에서 적용 게이트를 모두 체크한 뒤에만 완료한다.

- `[B]` 빌드/정적 검사 통과
- `[T]` 관련 자동 테스트 및 필요한 경우 전체 테스트 통과
- `[E]` 결과 상태를 단언하는 GPUI 상호작용 또는 필요한 실제 OS 검증 통과
- `[V]` 필요한 화면 확인 1회 또는 적용 불필요 사유; 독립 재조작은 기본 요건이 아님
- `[R]` 오류 리뷰 완료 및 오류 없음/잔여 오류 기록
- `[C]` 검증 결과를 커밋하고 `origin/main`까지 푸시

`TODO.md`에서 항목을 삭제하는 시점은 `[B]`, `[T]`, `[R]`, `[C]`와 해당 작업의 `[E]`/`[V]`
게이트가 모두 끝난 뒤다. 완료 결과는 `MASTER_PLAN.md`에 한 번 기록하고, 테스트 이름·명령·
캡처 경로·커밋을 중복해서 여러 문서에 복사하지 않고 이 문서의 증거 칸에서 참조한다.

사용자 수동 확인은 해당 OS/화면 조건의 `USER_ACCEPTED` 증거로 기록한다. 수행하지 않은
실제 조작을 통과했다고 쓰지 않으며, 아직 필요한 OS 검증은 `N/A`로 생략하지 않는다.

## 2026-09-29 Phase O 격리 E2E 보강

Windows Codex의 `CLAUDE_LOCAL` 하네스를 사용했다. 사용자 앱 PID 44832는 그대로 두고,
실행 중임을 읽기 전용 상태 조회로 확인한 `TACS` VM의 VDI는 열지 않았다. 이미 있는
VirtualBox 생성 5 MiB 합성 NTFS VDI를 `-SeedVdi`로 작업별 임시 루트에 복사했다.
릴리즈 바이너리 SHA-256은 `67D9F1F6F219F2327427D3447A98B8E9EAE7D11B09B66202257480987667F80F`다.
`cargo test -p gpui-convenience-tools --locked virtual_disk` 결과는 81 passed·3 ignored였다.

- `VDE-024`·`VDE-019`: 1200×900 초기 캡처 `phase-o-offline-initial-20260929-160012.png`에서
  루트 트리·파일 목록과 숨김/시스템 폴더를 확인했다. `-InitialGuestPath '$Extend'` 캡처
  `phase-o-subfolder-20260929-160715.png`에서는 현재 경로가 `$Extend`로 바뀌고 그 자식
  `$ObjId`가 표시됐다. 이는 시작 상태를 주입한 E2E이며 실제 트리 클릭 증거는 아니다.
- `VDE-012`·`VDE-015`·`VDE-019`: `-AutoCopyVdi` 격리 릴리즈 캡처
  `phase-o-offline-autocopy-20260929-160303.png`에 `완료 · 파일 17개 · 915.3 KB · 실패 1`과
  손상 `many_subdirs` 사유가 표시됐다. 격리 대상 실제 파일은 17개·937,234바이트였고
  13개가 숨김 또는 시스템 속성이었다. 실제 오류 억제 버튼 클릭·자연 속도 대용량 중지
  조작은 수행하지 않았다.
- `VDE-017`: 합성 미지원 파티션 VDI 캡처
  `phase-o-unsupported-20260929-160842.png`에서 NTFS 3.1만 지원한다는 오류·파티션
  경고를 확인했다. 실행 중 VM 판정은 자동 테스트
  `detects_a_running_vm_disk_from_machine_readable_info`로만 확인했고 실제 VM의 VDI를
  열어 보는 E2E는 사용자 결정에 따라 적용하지 않는다.

각 `Start`의 `launchFocusRestore`는 `skipped-focus-changed`였으므로 전역 Click/Key/Drag는
보내지 않았다. 따라서 `VDE-012`·`VDE-014`·`VDE-016`·`VDE-023`·`VDE-025`의 실제 입력
게이트는 그대로 남는다. 네 구현 검증 세션은 각각 `Stop` 후 임시 루트 제거를 확인했다.

독립 Visual Reviewer도 같은 SHA-256 릴리즈를 다른 격리 세션으로 시작해 GPUI 관련
테스트 70 passed·3 ignored를 재실행했다. 자체 첫 캡처
`independent-vde019-root-first-1200x900-161356.png`에서 NTFS 3.1·숨김/시스템 항목과
트리/목록 경계, `independent-vde019-extend-1200x900-161439.png`에서 `$Extend` 자식
`$ObjId`, `independent-vde019-autocopy-1200x900-161718.png`에서 복사 17개·실패 1건을
독립적으로 재확인했다. 대상 실제 파일은 17개·937,234바이트였고 구현 검증과 불일치는 없었다.
그러나 세션의 `launchFocusRestore: failed`와 사용자 작업 상태 불확실성 때문에 전역 입력을
보내지 않았으므로 종합 판정은 `BLOCKED(input)`이다. 실제 클릭·키보드·버튼 동작의 독립
검증을 `PASS`로 승격하지 않는다. 검토자의 세 세션은 모두 `Stop`으로 정리했고 사용자 앱
PID 44832는 유지됐다.

## 2026-09-30 필수 검증 기준 재판정

사용자 요청에 따라 공통 완료 기준을 실용적으로 조정했다. 과거의 실제 외부 입력·독립
재조작 미수행 기록은 그대로 보존하며, 새 검증을 수행했다고 주장하지 않는다.
축약 전 상세 증거는 커밋 `1f5320f`의 이 문서에 보존한다.

기존 이벤트·상태 테스트와 실제 릴리즈 증거로 `VDE-012`·`VDE-024`·`VDE-022`·`VDE-019`를
완료 기록으로 옮겼다. Code Reviewer가 실제 단언을 확인했고, `VDE-017`의 실행 중 VM 가드는
경로 일치뿐 아니라 열기 거부·소스 미연결을 확인해야 하므로 계속 활성으로 유지했다.
`VDE-014`·`VDE-015`·`VDE-016`·`VDE-025`의 버튼/수정키/저장 연결 단언과 `VDE-023`의 OS
대화상자 선택·취소는 남는다. 테스트 추가·실제 UI 재실행은 이번 문서 변경의 범위가 아니다.
별도 구조 항목 `G-001`도 기존 bounds 테스트와 릴리즈 두 화면 증거로 종료했다.
`C-002`는 기존 사용자 프로필 확인만, `G-003`은 드래그 저장·재로드 단언만 남긴다.

이번 문서 전용 변경은 `Assert-ProjectDocs.ps1`의 `DOCS_VERIFIED active=17 matrix=17`,
에이전트 TOML 3개 파싱, 정본·절차 참조 경로 확인, `git diff --check`를 통과했다.
제품 코드·설정·실행 중 앱은 변경하지 않았고 코드 테스트·새 화면 E2E는 실행하지 않았다.

## 2026-09-30 Phase O 수용 범위와 O-5 실제 검증

사용자는 현재 보이는 영역의 정상 표시를 우선하며 추가 입력 테스트 대부분의 필요성을
느끼지 않는다고 명시했다. `VDE-014`·`VDE-015`·`VDE-016`·`VDE-025`의 추가 조합·버튼
연결·대상 폴더 대화상자·단축키·드래그 저장 단언은 **선택 보강으로 전환**했다.
기존 기능과 저장 계약·복사 엔진의 취소 및 부분 파일 정리 테스트는 그대로 유지한다.
이 전환은 미수행 테스트의 `PASS`나 사용자 직접 조작 `USER_ACCEPTED`가 아니다.
이전 필수 조건과 증거는 커밋 `74b4a49`의 이 문서에 보존한다.

### 안전 E2E — VDE-017 PASS

앱 제품 코드는 바꾸지 않고 기존 `target/release/gpui-convenience-tools.exe`를 사용했다.
SHA-256은 `67D9F1F6F219F2327427D3447A98B8E9EAE7D11B09B66202257480987667F80F`다.
`GPUI_CONVENIENCE_TOOLS_DATA_DIR`에 격리하고 광고·자동 동기화를 끈 뒤 합성 디스크와
테스트 전용 VBoxManage 대역으로 실제 릴리즈의 시작→조회 프로세스→거부 화면을 검증했다.
실제 실행 중인 TACS 디스크를 여는 시도는 하지 않았다.

- 실행 중 VM 응답: `list runningvms`→`showvminfo ... --machinereadable` 2회 호출,
  실행 중 사용 안내·미연결 헤더·파티션 없음 확인. PID 33900.
  증거: `target/visual-validation/o5-running-2008c24274864cba95ef091b7125d782/running.png`.
- 조회 실패: 대역 exit 7·`O5_PROBE_QUERY_FAILURE`, 조회 실패 차단과 파티션 없음 확인.
  PID 21840. 증거:
  `target/visual-validation/o5-queryfailure-5bbbe26044534c1b80eaf753d1f31318/query-failure.png`.
- 잠금 표식: `fixture.vdi.lck`에서 조회 호출 0회, 잠금 사유·미연결·파티션 없음 확인.
  PID 17748. 증거:
  `target/visual-validation/o5-locked-bf2f85e5929d4b8ab81c828eb550a4c0/locked.png`.
- 각 세션에서 원본 SHA-256·길이·수정시각 불변과 임시 대상 빈 상태를 확인했다.
  `Assert`는 프로세스 조회·파일 불변만 단언하며 앱의 거부·미연결 판정은 위 실제 화면으로
  확인했다. 내부 `source=None` 값을 테스트 API로 직접 단언한 것은 아니다.
  초기 세션은 대상 입력과 임시 폴더가 연결되지 않았으므로 빈 폴더만으로 복사 차단을
  증명하지 않는다. 이 연결은 후속 하네스 리뷰에서 보강했다.

초기 하네스의 file log 단언은 실패했다. `push_log`가 이 메시지를 UI 로그에만 넣는 것을
확인하고, 파일 로그를 근거로 앱 상태를 단언하지 않도록 수정했다. 조회 trace 1행의
PowerShell scalar/array 차이도 수정한 뒤 조회 실패 시나리오 assertion을 다시 통과했다.
후속 리뷰에서 하네스의 임의 이미지 입력을 제거하고 동봉 NTFS fixture 생성으로 고정,
reparse-point 차단·동일 Process 핸들 종료·manifest 저장 실패 정리·앱 복사 대상 연결을
보강했다. 보강 후 `Prepare`의 fixture export·대역 컴파일은 통과했지만, 사용자 입력
감지 후 UI 실행을 중단했으므로 보강된 Start/Stop을 새 실제 앱으로 재실행한 것은 아니다.

### 파일 선택 — VDE-023 PENDING(input)

격리 PID 21100의 찾아보기 버튼으로 실제 네이티브 열기 대화상자와 파일 이름·선택·취소
컨트롤 표시를 확인했다. 입력 도구가 부모 앱과 소유 대화상자의 대상 좌표를 일치시키지
못했다. `set_value`는 `Cannot set a value for an element that is not settable`,
취소 클릭은 `point ... is over ... "탐색 단추", not target window ...`로 실패했다.
경로 선택과 취소 성공은 주장하지 않는다. 남은 확인은 종료된 VM의 VDI 선택→경로 반영,
다시 찾아보기→취소→동일 경로 유지 각 1회다. 제품 결함으로 재현된 것은 아니다.

커서 복원 API는 제공되지 않았다. 이전에 관찰한 탐색기 포커스 복원 시도도
`user input was detected in this window; call get_window_state before continuing`로
거부되어 추가 UI 조작을 중단했다. 포커스·커서 복원 성공으로 기록하지 않는다.
4개 검증 PID는 모두 종료했고 사용자 앱 PID 8488은 유지했다. 증거 루트는 삭제하지 않았다.

### 재실행과 자동 검증

```powershell
# Prepare는 fixture·대역만 만들며 앱 UI를 실행하지 않는다.
pwsh -NoProfile -File scripts/Test-VdiSafetyE2E.ps1 -Action Prepare
# 아래 Start는 실제 검증 창을 띄운다. 출력 sessionPath로 Assert/Stop을 수행한다.
pwsh -NoProfile -File scripts/Test-VdiSafetyE2E.ps1 -Action Start -Scenario Running
pwsh -NoProfile -File scripts/Test-VdiSafetyE2E.ps1 -Action Assert -SessionPath <session.json>
pwsh -NoProfile -File scripts/Test-VdiSafetyE2E.ps1 -Action Stop -SessionPath <session.json>
```

`QueryFailure`·`Locked`도 같은 절차로 실행한다. 실제 화면의 거부/미연결 확인을 생략하고
`O5_PROBE_ASSERT_PASSED`만으로 종합 PASS를 내지 않는다.
`cargo test -p gpui-convenience-tools virtual_disk --locked`는 **81 passed·3 ignored**,
`Prepare`의 동봉 fixture export는 **1 passed**다. ignored는 실제 이미지 주입용이므로
실행 중 VM 파일을 주입해 해제하지 않았다. 제품 소스 구조는 67파일·25,452줄로 변경 없다.

`cargo check -p gpui-convenience-tools --locked`, PowerShell parser(오류 0),
테스트 대역 `rustc --edition=2021 -D warnings`·`rustfmt --check`, 문서 assertion
`DOCS_VERIFIED active=12 matrix=12`, 구조 assertion과 `git diff --check`를 통과했다.
새 제품 코드 변경이 없어 사용자 앱 종료·릴리즈 재빌드는 수행하지 않았다.

읽기 전용 Code Reviewer의 안전 코드·문서 정합성 검토를 통과했다. 검증 도구·증거 및
선택 범위 변경은 커밋 `f91dc66`으로 `origin/main`까지 푸시했고 로컬/원격 해시 일치를
확인한 뒤 아래 C 게이트를 체크했다. 리뷰어의 새 실제 UI 재현을 주장하지 않는다.

## 작업별 체크 매트릭스

`TODO.md`의 모든 활성 ID는 이 표에 정확히 한 번 있어야 한다. `—`는 작업 성격상 해당
게이트가 적용되지 않음을 뜻하고, 적용 게이트는 `[ ]`에서 `[x]`로 갱신한다.

| ID | 프로필 | B | T | E | V | R | C | 증거 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C-002 | E2E | [x] | [x] | [ ] | [x] | [x] | [x] | 현재 AppData 구형 JSON 끝의 중괄호 초과를 읽기 전용으로 확인. 격리 복구·원본 불변·전체 설정/색상 모드 왕복·손상 설정 보존·동시 변경·새 설정 우선순위 및 GPUI Light/Dark 버튼 입력 테스트 추가. `config/persistence.rs`는 동시 갱신을 직렬화하고 임시 파일 완성 후 교체; 설정 읽기/저장 실패는 앱 로그에 표시. 전체 199 passed·7 ignored, `cargo check --locked`, Clippy `-D warnings`, 문서·구조 검사 통과. 사용자 승인 후 구 릴리즈를 정상 종료하고 release SHA-256 `67D9F1F6F219F2327427D3447A98B8E9EAE7D11B09B66202257480987667F80F` 빌드. `scripts/Test-SettingsPersistence.ps1`가 격리 프로세스 30956·31028을 차례로 시작/정상 종료해 구형 손상 파일 불변, 새 설정 파일 생성, 광고 자리 회수=true·색상 모드=dark·스캔 37초·서비스/동기화 off·즐겨찾기 유지 확인(`SETTINGS_E2E_PASSED`). 1000×700 실제 릴리즈 설정 화면 `target/visual-validation/captures/c002-dark-settings-restart-150840.png`에서 어두운 모드·카드 경계 확인; 검증 세션과 프로세스 정리. 2026-09-30 표시 검증은 기존 증거로 충족. 남은 일: 기존 사용자 프로필에서 광고 자리 회수를 다시 선택한 뒤 재시작 유지 여부 1회 확인. 격리 검증을 사용자 프로필 확인으로 표시하지 않음. |
| VDE-023 | GPUI | [x] | [x] | [ ] | [x] | [x] | [x] | 2026-09-30 실제 release의 찾아보기 클릭으로 네이티브 열기 대화상자를 표시했다. `set_value`는 not settable 오류, 좌표/키 입력 후 파일 이름 포커스·값 불변, 취소 클릭은 부모/소유 창 target mismatch로 차단됐다. 선택→경로 반영·취소→기존 경로 유지 각 1회는 미검증. 검증 PID 21100만 종료했고 사용자 PID 8488은 유지. |
| E-001 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-002 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-003 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-004 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| K-001 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| G-003 | E2E | [x] | [x] | [ ] | [x] | [x] | [x] | `sidebar_width` 설정 필드와 200~360px 보정·기본 240px 복원; `sidebar_width_is_normalized_to_supported_range`, `update_config_preserves_unedited_fields`, GPUI `sidebar_divider_drag_resizes_navigation_and_content`; 전체 최신 테스트는 상단 품질 게이트 기준 161 passed·4 ignored; 격리 release 기본 캡처 `target/visual-validation/captures/g003-default-003709.png`, `sidebar_width=320` 시드 복원 캡처 `target/visual-validation/captures/g003-restored-width-003742.png`; 최신 release 시작 캡처 `target/visual-validation/captures/next-g003-before-065645.png`에서 divider 위치를 재확인했으나 `-Action Drag -X 0.25 -Y 0.50 -ToX 0.32 -ToY 0.50`은 포그라운드 안전 검사(`현재 포그라운드=66246`)에서 차단되어 입력을 보내지 않음; 2026-09-30 표시 검증은 기존 증거로 충족. 남은 일: 저장을 활성화한 GPUI drag/mouse-up→설정 재로드→새 화면 폭 복원 단언. 외부 입력 반복·독립 재조작은 선택 |
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

`VDE-014`·`VDE-015`·`VDE-016`·`VDE-025`는 추가 검증을 선택 범위로 옮긴 기록이다.
해당 행의 E `—`는 이번 수용 범위에서 제외했다는 뜻이며 이전 미검증 사실을 지우지 않는다.

완료 작업은 `TODO.md`에서 제거하고 이곳에 게이트 결과를 보존한다. 상세 설계와 완료 단계는
`MASTER_PLAN.md`에 한 번만 기록한다.

| ID | 프로필 | B | T | E | V | R | C | 증거 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| VDE-017 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 2026-09-30 실행 중 VM 모의 응답·조회 실패·잠금 표식 3개 실제 릴리즈 차단 화면과 원본 불변/조회 trace를 확인. 위 O-5 증거 참조. 실제 실행 중 VM 디스크는 열지 않음. 관련 자동 테스트 81 passed·3 ignored. |
| VDE-025 | E2E | [x] | [x] | — | [x] | [x] | [x] | 컬럼 폭 보정·설정 round-trip·조절 버튼·bounds 테스트와 기존 릴리즈 캡처 통과. `virtual_disk_tree_width_setting_is_clamped_and_persisted`는 setter 호출이며 저장 비활성 상태라 drag 저장의 증거가 아님. 선택 보강: GPUI drag/mouse-up→설정 저장·재로드와 새 화면 폭 복원 단언.  2026-09-30 사용자 요청으로 추가 검증을 필수 범위에서 제외. 미수행을 PASS로 표시하지 않음. |
| VDE-014 | GPUI | [x] | [x] | — | [x] | [x] | [x] | 기존 더블클릭 폴더 진입 GPUI 이벤트·단순/Ctrl 선택과 Shift 선택 헬퍼 테스트 통과. 선택 보강: modifier가 있는 실제 GPUI 행 이벤트의 선택 집합과 상위 버튼 경로·목록 단언. 외부 키보드 반복 검증은 요건에서 제외.  2026-09-30 사용자 요청으로 추가 검증을 필수 범위에서 제외. 미수행을 PASS로 표시하지 않음. |
| VDE-015 | GPUI | [x] | [x] | — | [x] | [x] | [x] | 청크 취소·부분 파일 정리 자동 테스트, 실제 합성 VDI 복사와 제어된 취소 증거는 기존 기록 참조. 선택 보강: 대상 입력→시작/중지 버튼의 작업·취소 상태 자동 단언 및 대상 폴더 대화상자 선택/취소 1회. 자연 속도 초대형 파일 검증은 불필요.  2026-09-30 사용자 요청으로 추가 검증을 필수 범위에서 제외. 미수행을 PASS로 표시하지 않음. |
| VDE-016 | GPUI | [x] | [x] | — | [x] | [x] | [x] | Ctrl+A 선택은 GPUI 결과 단언으로 검증됨. 기존 종합 단축키 테스트는 빈 화면의 요소 존재만 확인한다. 선택 보강: 로드된 목록의 Enter/Backspace/F5/Ctrl+C 결과 및 입력창 포커스 시 비작동 단언.  2026-09-30 사용자 요청으로 추가 검증을 필수 범위에서 제외. 미수행을 PASS로 표시하지 않음. |
| G-001 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `ButtonStyle` 기본값 통일 및 개별 덮어쓰기 제거; 검증 전용 `-InitialPanel`로 패널 전환 입력을 격리 실행에 주입; `cargo check -p gpui-convenience-tools --locked`; 관련 GPUI 테스트 `ad_block_cards_contain_long_content_at_supported_widths`, `service_rows_keep_names_readable_at_supported_window_widths`, `file_sync_sections_share_one_width_at_every_window_width`; 전체 테스트 145 passed·4 ignored; Clippy exit 0(기존 경고 7건); 격리 release 파일 동기화 캡처 `target/visual-validation/captures/g001-file-sync-013019.png`와 자동 시작 캡처 `target/visual-validation/captures/g001-auto-start-013004.png`에서 두 화면과 공용 버튼 스타일 확인, processCount=0·sessionCount=0; 2026-09-30 기존 bounds·릴리즈 화면 증거와 읽기 전용 코드 리뷰로 완료 재판정(새 UI 실행 아님); commits `0d7bf25` 및 후속 검증 경로 커밋 push 완료 |
| VDE-024 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 지연 로딩 `GuestDirectoryTreeNode`와 좌우 `balanced_split` 탐색 영역을 추가해 폴더 트리 선택으로 중첩 경로를 바로 열고, 목록 행 높이·열 폭·탐색 카드 높이를 컴팩트하게 조정; `virtual_disk_folder_tree_navigates_nested_paths_without_repeated_list_clicks`, `virtual_disk_explorer_keeps_tree_and_file_list_inside_compact_card`와 전체 169 passed·4 ignored, Clippy `-D warnings` 통과; `Verify-Workspace.ps1` 필수 GPUI 테스트 29개 이름 검증; release 캡처 `target/visual-validation/captures/vde024-tree-920x700-121233.png`, `vde024-tree-1000x700-121200.png`, `vde024-tree-1280x900-121234.png`; 실제 트리 Click은 하네스 좌표 변환으로 상태 전환을 확정하지 않아 GPUI 이벤트 테스트로 대체, commit `4af7cc6` push 완료; 2026-09-30 새 기준으로 완료: 실제 GPUI 트리 클릭→직계 경로/목록 단언과 기존 캡처가 충분 |
| VDE-022 | RUST | [x] | [x] | [x] | [x] | [x] | [x] | 실제 `D:\VMMachine\win11\TACS\TACS_1.vdi`를 읽기 전용으로 대조해 `VDI normal (base)`, `dynamic default`, GPT 1번 MSR, 2번 `-FVE-FS-` BitLocker를 확인; `classifies_bitlocker_and_microsoft_reserved_partitions_explicitly`, `encrypted_partition_error_explains_the_offline_boundary`, 전체 테스트 163 passed·4 ignored, release 캡처 `target/visual-validation/captures/vde022-final-095347.png`에서 MSR·BitLocker 라벨과 안내를 확인; commit `6b89a09` push 완료; 2026-09-30 새 기준으로 완료: 분류·안내 자동 테스트와 기존 릴리즈 화면 확인이 충분 |
| VDE-012 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 160 passed·4 ignored; `virtual_disk::copy::tests::collecting_copy_errors_removes_partial_output_and_keeps_issue`로 원본 단축 읽기 실패 뒤 부분 대상 파일 제거와 원본 `SourceChanged` 사유 보존을 확인하고, 오류 행 selector·억제 키 저장/재로드·미억제 토스트 게이트와 `virtual_disk_panel_renders_copy_progress_and_issue_summary`의 GPUI `simulate_click` 억제/재표시 dispatch를 검증; 실제 VBox 생성 VDI 릴리스 복사에서 `many_subdirs` 손상 사유·17개 파일 부분 결과를 확인; 2026-09-30 새 기준으로 완료: GPUI 억제/재표시 클릭·저장 단언과 릴리즈 오류 표시 증거가 충분하며 별도 외부 클릭은 선택 |
| VDE-019 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `cargo check -p gpui-convenience-tools --locked`; 최신 전체 `cargo test --all-targets --all-features --locked` 160 passed·4 ignored, VDI 집중 60 passed·2 ignored; 표준 VDI 헤더 오프셋 수정 후 VirtualBox 7.2.14 `convertfromraw` 생성 VDI(`Storage format: VDI`, `State: created`)를 `GPUI_CONVENIENCE_TOOLS_VBOXMANAGE` 환경 변수와 함께 격리 릴리스 앱에 주입; 숨김/시스템 항목 17개·937,234바이트 복사와 `many_subdirs` 손상 사유 확인; 최신 release 1200×1000 캡처 `target/visual-validation/captures/continuation-vde019-current-release-040648.png`에서 숨김/시스템 행과 17개·915.3KB·실패 1건 요약을 재확인; 이번 구현 세션에서도 1200×1000 `target/visual-validation/captures/continuation-current-vde019-1200x1000-052717.png`와 920×700 `target/visual-validation/captures/continuation-current-vde019-920x700-052734.png`를 캡처해 안전 경계·NTFS 3.1·숨김/시스템 행·16개 선택과 compact 폭 경계를 확인; 선택 전 `0개 선택` 캡처와 `-SelectAllVdi` 후 `16개 선택` 캡처 `target/visual-validation/captures/vde019-vbox-key-before-024822.png`, `vde019-vbox-select-all-seed-025058.png`; 기본 포커스 전환·UI Automation `SetFocus`·`SetWindowPos` 모두 foreground를 바꾸지 못해 `Key Ctrl+A`는 안전 차단으로 미검증; 세션 종료 후 `PROCESS_COUNT=0`·`SESSION_COUNT=0`·대상 루트 삭제; 2026-09-29 격리 복사 17개·937,234바이트 및 독립 증거는 위 기록 참조. 2026-09-30 목록·호스트 복사 통합 범위만 완료로 재판정; 선택·단축키·복사 버튼·안전 가드의 잔여 단언은 VDE-014~017에서 추적; commits `87937dc`, `bed5914` |
| AD-006 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | `kakao_layout.rs`의 창 계층·크기 지문 및 6개 HWND 적용/역순 복원, GPUI 경계 테스트와 전체 199 passed·7 ignored, 릴리즈 빌드·Clippy `-D warnings` 통과(기존 기록). 실제 KakaoTalk 전후 캡처 `target/visual-validation/captures/ad006-before.png`·`target/visual-validation/captures/ad006-reclaimed.png`와 사용자 실기기 확인을 근거로 Phase A 종료. 독립 Visual Reviewer 세션은 수행하지 않았고, 이번 사용자 직접 수용을 V 게이트의 명시적 예외로 기록한다. 새 데스크톱 테스트를 수행했다는 뜻이 아니다. |
| VDE-021 | DECISION | — | — | — | — | [x] | [x] | 2026-09-29 사용자 결정: 실행 중 VM 파일 접근은 VDI 직접 읽기와 `VBoxManage guestcontrol` 모두 구현하지 않는다. O-6을 종료했고, 과거 분리 설계(commit `28dc516`)는 `VIRTUAL_DISK_BACKENDS.md`에 미채택 이력으로 보존한다. 실행 중 VM 파일을 검증용으로 열거나 Guest Control E2E를 수행하지 않았다. |
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
| D-020 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 최근 이력 상태·건수·소요 시간 단위 테스트와 920×480·994×702·1280×900 카드 경계 GPUI 테스트 통과. 최소 높이에서 페이지 overflow·wheel offset·마지막 `sync-history-row-2`의 뷰포트 진입 단언을 추가해 독립 검토의 최초 `FAIL` 사유를 해소. 전체 186 passed·5 ignored, 필수 GPUI 34개, Clippy `-D warnings`. 독립 Visual Reviewer가 제품 commit `478a4c4`의 release SHA-256 `21D17114A4FBC90C6D5AF9B6979A91F976B69182CDD550E88A589A2C49FE6866`을 별도 격리 실행하고 자체 캡처 `d020-independent-first-920-160505.png`, `d020-independent-920-compact-bottom-161016.png`, `d020-independent-994-compact-bottom-161030.png`, `d020-independent-1280-compact-bottom-161030.png`에서 스크롤바 드래그로 마지막 이력·건수·소요 시간·경계를 확인한 뒤 보강 테스트를 독립 재실행해 최종 `PASS` 판정. 실제 첫 wheel 입력 2회는 화면을 이동시키지 않아 성공으로 주장하지 않으며, 검증 PID·격리 루트 제거를 확인. |
| D-014 | E2E | [x] | [x] | [x] | [x] | [x] | [x] | 기본 Skip, 원본 내부 Follow, 대상 내부 상대 링크 Recreate 정책·경고를 확정. 숨김·시스템 항목 포함 직접 선택과 링크 모드 GPUI 테스트, 원본 밖·순환·대상 링크 탈출 테스트 통과. 격리 release SHA-256 `4EEA9866630D195F149314AF466CD08C475D6457CDE125545264DA55CF26D2B9` 캡처 `d014-file-sync-full-2200-150006.png`(1280px)·`d014-file-sync-920-full-150327.png`에서 숨김/시스템 폴더·제외 버튼·링크 모드·카드 경계 확인; 검증 PID/세션 루트 0, 임시 대상 미생성. 구현 commit `c94b74f` push 완료. |
| D-015 | RUST | [x] | [x] | — | — | [x] | [x] | Windows 파일/폴더 심볼릭 링크와 정션의 Follow·Recreate 구현, 동일 링크 재실행 건너뜀 및 다른 링크·일반 파일 충돌 비덮어쓰기 테스트. 전체 186 passed·5 ignored, 관리자 권한 링크 테스트 exit 0, Clippy `-D warnings` 통과. 구현 commit `c94b74f` push 완료. |
| D-016 | RUST | [x] | [x] | — | — | [x] | [x] | 링크 생성 오류 1314를 관리자 권한/개발자 모드 안내가 있는 경로별 실패로 변환. 관리자 권한 테스트 바이너리 `sync::link_tests` exit 0. 권한 없는 1314 실기계 조건은 재현되지 않아 오류 생성 단위 테스트로 메시지를 검증. 구현 commit `c94b74f` push 완료. |
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
