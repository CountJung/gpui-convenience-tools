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

절차와 실행 결과를 구분한다. 이전 코드의 테스트 수·해시·실제 화면과 미검증 경계는
[검증 결과 기록](VERIFICATION_RECORDS.md)에 보존한다. 문서 정리만으로 제품을 재검증하지 않는다.

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

#### 문서 전용 검증

저장소 루트에서 실행한다. 설치 경로가 다르면 실제 전역 checker 경로를 사용한다.

```powershell
$checker = Join-Path $env:USERPROFILE '.agents/skills/docs-canon/scripts/check_docs.py'
if (-not (Test-Path -LiteralPath $checker)) { throw 'docs-canon checker 설치 위치를 확인하세요.' }
python $checker . --registry docs/README.md
python $checker . --registry docs/README.md --changed
pwsh -NoProfile -File .\scripts\Assert-ProjectDocs.ps1
git diff --check
```

- 형식: 상대 파일 링크·표·등록부에서의 재귀 도달성과 열린 ID 대응을 확인한다.
  긴 줄은 경고다. 종료 0은 서술이 맞다는 뜻이 아니다.
- 내용: 현재 설명과 변경 근거, 남은 일, 고유 근거 보존·중복 여부를 따로 검토한다.
  이관 시 제목/명시 앵커와 상대경로도 대조한다. checker는 앵커를 판정하지 않는다.
- 구조 수치에 영향을 줬다면 `Assert-ProjectStructure.ps1`도 실행한다.
  제품 소스·설정·UI가 바뀌지 않은 회차는 빌드·제품 테스트·E2E를 미실행으로 보고한다.

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
게이트가 모두 끝난 뒤다. 완료 단계는 `IMPLEMENTATION_HISTORY.md`, 테스트 이름·명령·
캡처 경로·커밋은 `VERIFICATION_RECORDS.md`에 한 번 기록하고 증거 칸에서는 링크한다.
외부 푸시는 사용자 승인 범위에서만 수행한다. 승인 없이 푸시하거나 `[C]`를 통과 처리하지 않는다.

예외: 사용자가 기능 확인 후 **수용 종료를 명시**하면 해당 기능은 열린 목록에서 이관할 수 있다.
그때 사용자 확인 내용과 기존 검증 근거를 완료 기록으로 연결하고, 미수행 `[C]`는 반영 대기로
명시한다. 기능 수용과 Git 반영을 구분하는 예외이며 검증 통과·커밋·푸시 승인을 만들어 내지 않는다.

사용자 수동 확인은 해당 OS/화면 조건의 `USER_ACCEPTED` 증거로 기록한다. 수행하지 않은
실제 조작을 통과했다고 쓰지 않으며, 아직 필요한 OS 검증은 `N/A`로 생략하지 않는다.

## 작업별 체크 매트릭스

`TODO.md`의 모든 활성 ID는 이 표에 정확히 한 번 있어야 한다. `—`는 작업 성격상 해당
게이트가 적용되지 않음을 뜻하고, 적용 게이트는 `[ ]`에서 `[x]`로 갱신한다.

| ID | 프로필 | B | T | E | V | R | C | 증거 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C-002 | E2E | [x] | [x] | [ ] | [x] | [x] | [x] | [기존 설정 검증](VERIFICATION_RECORDS.md#c002-settings-evidence). 사용자 프로필 재시작 확인 미완료. |
| VDE-023 | GPUI | [x] | [x] | [ ] | [x] | [x] | [x] | 2026-09-30 실제 release의 찾아보기 클릭으로 네이티브 열기 대화상자를 표시했다. `set_value`는 not settable 오류, 좌표/키 입력 후 파일 이름 포커스·값 불변, 취소 클릭은 부모/소유 창 target mismatch로 차단됐다. 선택→경로 반영·취소→기존 경로 유지 각 1회는 미검증. 검증 PID 21100만 종료했고 사용자 PID 8488은 유지. |
| E-001 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-011 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| E-012 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-003 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| E-004 | E2E | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | — |
| K-001 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | — |
| G-003 | E2E | [x] | [x] | [ ] | [x] | [x] | [x] | [기존 폭 검증](VERIFICATION_RECORDS.md#g003-splitter-evidence). GPUI drag/mouse-up 저장·복원 단언 미완료. |
| K-002 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | macOS 메뉴 막대 지원은 macOS 실행 환경 확인 후 진행 |
| K-003 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | `launchd` 자동 시작은 macOS 실행 환경 확인 후 진행 |
| K-004 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | `.icns` 배포 아이콘은 macOS 패키징 환경 확인 후 진행 |
| K-005 | RUST | [ ] | [ ] | — | — | [ ] | [ ] | 코드 서명·공증은 Apple Developer 계정 확보 후 진행 |


## 완료 검증 기록

완료 항목과 과거 실행 근거는 [검증 결과 기록](VERIFICATION_RECORDS.md)의 단일 정본으로 이동했다.
새 검증 결과는 그 문서에 기록하고, 이 문서에는 현재 절차와 열린 게이트만 유지한다.

<a id="vde-026-workspace-evidence"></a>

### 탐색 작업 화면 증거

기존 앵커의 호환 경로다. [해시별 실제 증거와 확인 한계](VERIFICATION_RECORDS.md#vde-026-workspace-evidence)를 따른다.
