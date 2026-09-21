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
| VDE-004 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-005 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-006 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-007 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-008 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-009 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-010 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-011 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-012 | E2E | [ ] | [ ] | [ ] | — | [ ] | [ ] | — |
| VDE-013 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| VDE-014 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| VDE-015 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| VDE-016 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| VDE-017 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| VDE-018 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| VDE-019 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| VDE-020 | DOCS | — | — | — | — | [ ] | [ ] | — |
| VDE-021 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-001 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-002 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-003 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-004 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |

| D-005 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-006 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-007 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-008 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| D-009 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-010 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| D-011 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-012 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-013 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| D-014 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-015 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-016 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-017 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-018 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-019 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| D-020 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-001 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-002 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-003 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-004 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| G-001 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| G-002 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| K-001 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| G-003 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| G-004 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| T-001 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| T-002 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |

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
