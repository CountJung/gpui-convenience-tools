---
name: gpui-visual-check
description: GPUI 레이아웃·시각 회귀를 조사하고 실제 화면으로 검증하는 실행 절차. UI를 바꿨거나, 사용자가 스크린샷으로 레이아웃 이상(폭이 안 맞음, 잘림, 스크롤 안 됨)을 보고했거나, 폭·높이·스크롤 회귀를 재현해야 할 때 사용한다. 창 캡처 하네스 운용, `debug_bounds` 진단 테스트, 재현 매트릭스, 정리 확인까지를 다룬다.
---

# GPUI 시각·레이아웃 검증 실행 절차

> **정책이 아니라 절차 문서다.** 어느 표면에서 무엇을 `PASS`로 판정하는지, 어떤 증거를
> 남기는지는 `.github/copilot-instructions.md`의 「실행 표면 하드 게이트」와
> 「GPUI 시각 검증 및 독립 크로스체크」가 정본이다. 여기에는 규칙을 다시 쓰지 않고
> **어떻게 하는가**만 적는다.

---

## 순서

1. **자체 테스트로 먼저 잰다** (표면 무관, 30초)
2. 재현되면 → 고치고 회귀 테스트로 남긴다
3. 재현되지 않으면 → **재현 매트릭스**를 돌린다
4. 실제 화면 검증 (표면별 실행)
5. 정리 확인 후 보고

화면부터 띄우지 않는다. 3단계에서 8가지 조건을 캡처로 훑는 것보다, 1단계에서 여러 창
너비의 좌표를 한 번에 찍는 편이 훨씬 빠르고 정확하다.

---

## 1. `debug_bounds` 진단 테스트 (먼저 할 것)

임시 `#[gpui::test]`를 만들어 요소의 실제 좌표·크기를 찍는다. 가설을 몇 초 만에 기각할 수
있고, 눈으로 픽셀을 세는 것보다 정확하다.

```rust
#[gpui::test]
fn diag_widths(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (_view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::FileSync));

    for w in [920.0f32, 994.0, 1280.0, 1600.0] {
        cx.simulate_resize(size(px(w), px(700.0)));
        refresh(cx);
        let vp = cx.debug_bounds("file-sync-page").unwrap();
        let card = cx.debug_bounds("file-sync-job-list-card").unwrap();
        println!("win={w} vp={:?} card={:?}", vp.size.width, card.size.width);
    }
}
```

```powershell
cargo test -p gpui-convenience-tools diag_widths -- --nocapture
```

지켜야 할 것:

- **여러 창 너비를 돌린다.** 컨텐츠가 뷰포트보다 넓으면 어차피 뷰포트 폭으로 잘려 정상으로
  보인다. 최소 지원 폭만 확인하면 **컨텐츠가 뷰포트보다 좁아지는 넓은 창에서만 나타나는
  폭 회귀를 통째로 놓친다.**
- 끝나면 **반드시 되돌린다** — `git checkout app/src/app/tests/<file>.rs`.
- 재현에 성공했다면 진단 테스트를 정식 회귀 테스트로 다듬어 남기고
  `scripts/Verify-Workspace.ps1`의 필수 테스트 목록에 이름을 추가한다.

### `debug_bounds`가 요소를 못 찾을 때

`ui::action_button`(`Stateful<Div>`)과 `ui::toggle_switch`(`Switch`)는 `debug_selector`를
달지 않는다. 테스트에서 클릭해야 하면 호출부에서 감싼다 — 저장소의 기존 관용구다.

```rust
div().debug_selector(|| "sync-apply-paths".to_string())
    .child(ui::action_button("sync-apply-paths", ...))
```

---

## 2. 재현 매트릭스 (사용자 보고를 재현하지 못할 때)

한 가지 조건만 보고 "정상"이라고 결론 내지 않는다. 아래를 순서대로 훑는다.

| 조건 | 왜 보는가 |
| --- | --- |
| 신규 실행 | 기준선 |
| 리사이즈(좁게 ↔ 넓게) | 스플리터 상태가 창 크기 변화를 못 따라가는 경우 |
| 최소화 → 복원 | 크기 0 프레임에서 잡힌 레이아웃이 남는 경우 |
| 디버그 빌드 | `debug_assertions` 분기(inspector 등) 차이 |
| 스크롤 후 | 스크롤 오프셋이 붙은 상태의 재배치 |
| 백그라운드 작업 중 | 잦은 재렌더·이벤트 폭주 상태 |
| 트레이 숨김 → 복원 | 창이 숨겨진 동안의 레이아웃 |
| 사용자와 같은 창 크기 | 스크린샷의 창 크기를 그대로 맞춘다 |

**전부 정상이면 "재현하지 못했다"를 먼저 보고한다.** 재현한 척하지 않는다.

### 스크린샷에서 역산하기

재현에 실패해도 스크린샷은 단서를 준다. 특히 **폭**은 역산이 잘 된다.

- 섹션마다 오른쪽 끝이 다르다 → 각 섹션이 **자기 내용 폭**으로 잡혔다는 뜻.
  각 섹션의 폭이 그 안의 가장 긴 요소(헤더 버튼 줄, 안내 문장)와 일치하는지 확인하면 확정된다.
- 형제 요소는 정상 폭인데 스크롤 안쪽만 좁다 → 스크롤 컨테이너가 자식에게 폭을 주지 못한 것.
- 글자가 잘리거나 두 줄로 접힌다 → 고정 폭(`w(px(..))`)이 라벨보다 작다.
  `min_w` + `whitespace_nowrap`으로 고치고, 같은 칸이 두 파일에 있으면 `ui.rs`로 승격한다.

가설에 따라 구조적으로 막았다면 **"가설 기반"임을 결과 보고와 `MasterPlan.md`에 명시한다.**

---

## 3. 실제 화면 검증 — `CLAUDE_LOCAL` (Windows Claude Code)

### 기본 흐름

```powershell
# 앱이 떠 있으면 exe가 잠겨 release 빌드가 실패한다. 항상 Stop → build 순서.
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
cargo build -p gpui-convenience-tools --release

scripts\Invoke-ClaudeVisualCheck.ps1 -Action Start -SeedConfig <config.json> -Width 994 -Height 702
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Click -X 0.12 -Y 0.65
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name after-click
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
```

캡처된 PNG는 Read 도구로 직접 연다. 경로는 `Capture`가 JSON으로 돌려준다.

### 좌표 지정

`-X`/`-Y`는 **클라이언트 영역 기준 0~1 비율**이다. 캡처 이미지에서 요소 중심 픽셀을 읽고
캡처 크기로 나누면 몇 px 오차 안에서 맞는다(캡처는 창 전체 rect이므로 테두리만큼 어긋난다).

- **요소 중앙을 노린다.** 목록 항목 사이 간격에 떨어지면 아무 일도 일어나지 않는다.
- `Click`/`Wheel`은 실제 화면 좌표를 `at`으로 출력한다. 빗나갔을 때 이 값으로 보정한다.
- **눌렀다고 가정하지 말고 다시 캡처해 확인한다.** 특히 패널 전환처럼 무거운 동작
  (예: 서비스 목록 조회)은 기본 대기(700ms)보다 오래 걸릴 수 있어 재캡처가 필요하다.

### 상태 재현 (`-SeedConfig`)

특정 화면(등록된 작업, 실패 목록, 특정 테마)을 재현하려면 `config.json`을 만들어 넘긴다.
격리된 데이터 루트에 복사된 뒤 앱이 그것을 읽는다.

- JSON을 Bash 힙독으로 쓰지 말 것 — 백슬래시가 먹혀 `Bad JSON escape`가 난다.
  Write 도구로 쓰거나 경로에 `/`를 쓴다.
- 시드가 잘못돼도 앱은 조용히 기본값으로 뜬다. **화면에 시드한 상태가 보이는지 먼저 확인한다.**

### 함정

| 함정 | 결과 | 대응 |
| --- | --- | --- |
| `-Action Stop`이 세션 루트(=격리 `appData`)를 통째로 지운다 | 같은 상태로 재시작 불가 | 재시작이 필요하면 **Stop 전에 `config.json`을 밖으로 복사**해 다음 `-SeedConfig`로 넘긴다 |
| 검증 앱이 `target\release\*.exe`를 잠근다 | `cargo build --release`가 `os error 5` | 코드를 고쳤으면 **Stop → build → Start** |
| 키보드 입력·드래그가 없다 | 텍스트 입력·divider 드래그 검증 불가 | **한계로 분리해 적고 그 항목을 근거로 `PASS`를 내지 않는다.** GPUI 자체 테스트로 덮는다 |
| `APPDATA`만 바꾸면 격리되지 않는다 | 사용자의 실제 config·로그를 건드린다 | 하네스가 쓰는 `GPUI_CONVENIENCE_TOOLS_DATA_DIR`를 그대로 쓴다 |
| `PrintWindow`가 합성 중간 프레임을 잡을 수 있다 | 가상 리스트 행이 겹쳐 보이는 등 실제와 다른 그림 | 잠시 뒤 재캡처해 같은 그림이 나오는지 확인한 뒤 결론 |
| 캡처는 정지 화면이다 | 애니메이션·순간 상태는 못 잡는다 | 상태 전이는 전/후 두 장으로 나눠 캡처 |

### 파괴적 동작

검증에서 실제 데이터를 건드리지 않는다. 동기화처럼 파일을 쓰는 기능을 확인해야 하면:

- 원본은 **읽기만 하는 폴더**, 대상은 **임시 폴더**로 시드한다.
- 오래 걸리는 실행은 중간에 `중지`를 눌러 끊는다.
- 끝나면 임시 대상 폴더를 지운다. 지워졌는지 `Test-Path`로 확인한다.

### 정리 확인

`-Action Stop` 뒤 세 가지를 확인하고 결과에 적는다.

```powershell
(Get-Process gpui-convenience-tools -ErrorAction SilentlyContinue).Count          # 0
(Get-ChildItem $env:TEMP -Directory -Filter "gpui-claude-visual-*").Count         # 0
Test-Path <검증용 임시 대상 폴더>                                                  # False
```

---

## 4. 실제 화면 검증 — `IDE` (VS Code Codex·Copilot)

이 표면에서는 화면 조작을 **시도하지 않는다.** `scripts/Verify-Workspace.ps1`로 자동 검증을
마치고 `IDE_VERIFIED / DESKTOP_PENDING`으로 인계한다. 위 1·2단계(진단 테스트, 재현 매트릭스의
코드 기반 항목)는 이 표면에서도 그대로 유효하며, 인계 전에 끝내 두면 데스크톱 검증에서
확인할 항목이 줄어든다.

---

## 5. 보고

- 캡처 PNG 경로를 증거로 남긴다. 기존 스크린샷이나 구두 설명으로 대체하지 않는다.
- 하네스가 덮지 못한 수용 기준을 **한계**로 따로 적는다.
- 재현하지 못한 보고는 시도한 조건과 결과를 나열하고, 넣은 대응이 확인된 수정인지
  가설 기반인지 구분한다.
- 판정은 `PASS` / `FAIL` / `BLOCKED` 중 하나. 한계 항목을 근거로 `PASS`를 내지 않는다.
