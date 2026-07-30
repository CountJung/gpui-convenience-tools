# GitHub Copilot Main Instructions

## 목적

이 문서는 gpui-convenience-tools 저장소의 코딩 지침 단일 정본이다.
모든 구현, 리뷰, 문서 갱신은 이 문서를 기준으로 수행한다.

## 프로젝트 정체성

**GPUI 기반 다용도 데스크탑 보조 도구(convenience tools) 모음.**

하나의 앱 안에 서로 독립적인 편의 기능들을 패널 단위로 추가해 나가는 구조다.
특정 기능(광고 차단 등)이 앱의 정체성이 아니라, **편의 기능을 담는 그릇**이 정체성이다.
따라서 새 기능을 추가할 때 기존 기능과 결합하지 말고 독립 패널로 만든다.

현재 편의 기능:

| 기능 | 사이드바 명칭 | 모듈 |
| --- | --- | --- |
| WebView 광고 창 숨김 | 웹뷰 광고 차단 | `window/ad_block.rs` |
| 폴더 → 폴더 주기 동기화 | 파일 동기화 | `window/file_sync.rs` |
| Win32 서비스 제어 | Windows 서비스 | `window/service_mgr.rs` |

## 적용 범위

- Rust 소스 코드 수정
- GPUI UI 구현
- gpui-component 테마 적용
- Windows 플랫폼 통합 코드
- 워크스페이스 설정 파일과 진행 문서 갱신

## 단일 정본 원칙

- 이 문서를 코딩 지침의 단일 정본으로 유지한다.
- 다른 문서에는 규칙을 중복 작성하지 않고, 이 문서를 링크한다.
- 규칙 변경은 먼저 이 문서에 반영한 뒤, 참조 문서는 링크만 유지한다.

## 구현 기준

- MasterPlan 단계 기준으로 작업 범위를 결정한다.
- 단계 목표를 벗어나는 아키텍처 변경은 사용자 요청이 없는 한 수행하지 않는다.
- 단계를 끝내면 `MasterPlan.md`「구현 완료 단계」에 결과를 적고, 그 단계에서 소화한 항목을
  `TODO.md`에서 지운다. 완료 이력은 MasterPlan, 미착수 대기열은 TODO가 정본이다.
  체크리스트를 복제한 별도 진행 문서는 두지 않는다(과거 `TASKS.md`는 제거됨).

## 편의 기능 패널 구조 기준

새 편의 기능은 다음 구조를 따른다.

1. `ActivePanel`에 variant를 추가한다.
2. `NAV_TOOLS`(app.rs)에 (패널, 명칭, 한 줄 설명)을 추가한다.
   명칭은 기능을 모르는 사용자도 이해할 수 있는 한국어로 짓는다.
3. `window/<기능>.rs`에 `pub fn render(this: &mut AppRoot, window, cx) -> AnyElement`를 만든다.
4. 패널 내부는 **스플리터(`h_resizable`)로 기능 영역과 설정 영역을 분리**한다.
   - 왼쪽: 기능 본체(목록, 상태, 실행 결과)
   - 오른쪽: 그 기능의 설정
   - 각 영역은 `window::scroll_pane`으로 감싸 자체 스크롤을 갖는다.
5. 앱 전역 설정(테마, 로그 보관)만 `window/settings.rs`에 둔다.
   기능별 설정을 전역 설정 페이지에 넣지 않는다.
6. `app.rs`의 `fills_height` 목록에 새 패널을 추가한다(스플리터는 높이를 스스로 채우므로).

## 구조 리팩터링 기준 (1,000줄 트리거)

**줄 수는 원인이 아니라 증상이다.** 파일이 1,000줄을 넘었다는 것은 그 파일이 길다는 뜻이
아니라, **책임 배치가 프로젝트 규모를 못 따라왔다는 신호**다. 따라서 1,000줄 초과 시
수행하는 것은 "파일 자르기"가 아니라 **기능 관점의 구조 재설계**다.

기계적 분할(줄 수를 맞추려고 앞뒤로 자르기, `part1.rs`/`part2.rs` 같은 이름)은 금지한다.
그렇게 하면 줄 수 지표만 내려가고 중복과 오배치는 그대로 남는다.

### 임계값

| 줄 수 | 조치 |
| --- | --- |
| ~800줄 | 주의. `PROJECTMAP.md`에 리팩터링 계획(중복 후보 포함)을 미리 적어 둔다 |
| 800~1,000줄 | 경고. 다음 작업 전에 구조 리팩터링을 수행한다 |
| 1,000줄 초과 | **즉시 리팩터링.** 다른 작업보다 우선한다 |

### 리팩터링 절차 — 순서를 지킨다

트리거된 파일 하나만 보지 않는다. **먼저 프로젝트 전체를 훑어 같은 책임이 어디에 흩어져
있는지 확인한 뒤**, 아래 순서로 처리한다. 1~2단계에서 이미 임계값 아래로 내려가면
3단계(분할)는 하지 않는다 — 파일 개수를 늘리는 것이 목적이 아니다.

1. **중복 제거 (공용 유틸 승격)** — 같은 책임의 헬퍼가 여러 파일에 복제돼 있으면
   먼저 하나로 합친다. 아래 「공용 유틸 승격 기준」을 따른다.
2. **오배치 책임 이동** — 그 파일이 소유하면 안 되는 로직을 원래 주인에게 돌려준다.
   - 순수 로직이 UI 파일에 있음 → `sync.rs` / `config.rs` 같은 도메인 모듈로
   - 패널 렌더가 `app/`에 있음 → `window/<기능>.rs`로
   - Win32 호출이 UI/도메인 코드에 있음 → `platform/windows/` 하위로
3. **책임 단위 분할** — 그래도 크면 같은 이름의 **디렉터리 모듈**로 바꾸고
   (`app.rs` → `app/mod.rs`) 책임 단위로 자른다. 경로 이동만으로 `use` 구문이 깨지지
   않아 호출부 수정이 최소화된다.
   - 순수 데이터 타입 → `state.rs`
   - 백그라운드 스레드 → `background.rs`
   - 이벤트 처리 → `events.rs`
   - 조작 메서드 묶음 → `<도메인>_ops.rs`
   - 패널의 좌/우 split 영역이 각각 크면 → `<기능>/list.rs`, `<기능>/settings.rs`

   `impl` 블록은 여러 파일에 나눠도 된다(같은 크레이트라면 허용).
   `mod.rs`에는 타입 정의와 생성자, 최상위 렌더만 남긴다.

### 리팩터링 수행 규칙

- **동작 변경 없이** 수행한다. 옮기거나 합치면서 로직을 고치지 않는다.
  리팩터와 기능 변경을 한 커밋에 섞지 않는다.
- 중복을 합칠 때 구현이 미세하게 다르면(`px_2` vs `px_3` 등) 어느 쪽으로 통일할지
  정하고, **그 통일은 별도 커밋**으로 분리한다. 리팩터 커밋에서 화면이 바뀌면 안 된다.
- 리팩터링 직후 `cargo check`(경고 0)와 `cargo test`로 동작 동일성을 확인한다.
- 결과를 `PROJECTMAP.md`에 반영한다(줄 수, 책임 설명, 리팩터링 이력, 공용 유틸 인벤토리).

### 예외

- 자동 생성 파일, 데이터 테이블(테마 JSON 목록 등)은 대상이 아니다.
- 쪼개면 오히려 응집도가 깨지는 경우(예: 하나의 Win32 API 시퀀스)는 분할하지 않고
  `PROJECTMAP.md`에 사유를 적는다.

## 공용 유틸 승격 기준

이 규칙은 1,000줄 트리거와 **독립적으로 상시 적용된다.** 중복은 파일이 커지기 전에
발견되므로, 임계값을 기다리지 말고 그 자리에서 승격한다.

### 즉시 판정은 즉시 처리한다

**「즉시」로 판정된 항목(3개 파일 이상 중복, 1,000줄 초과)은 발견한 작업 안에서 끝낸다.**
`TODO.md`에 적어 두고 넘어가지 않는다 — 대기열에 넣는 순간 중복은 계속 늘어나고,
나중에 합칠 때 검증해야 할 호출부만 많아진다. 대기열에 넣어도 되는 것은 **후보(2곳)** 뿐이다.

지금 처리하기에 범위가 너무 크다고 판단되면, 미루는 대신 **왜 미루는지와 언제 할지를
`PROJECTMAP.md`에 적는다.** 판단 근거 없이 미루지 않는다.

### 중복 판정

- **이름이 달라도 하는 일이 같으면 중복이다.** (`badge` / `state_badge`,
  `stat_card` / `stat_tile`처럼 한쪽은 색을 인자로 받고 한쪽은 `cx.theme()`에서
  읽는 정도의 차이는 같은 헬퍼로 본다.)
- 함수로 추출되지 않은 **인라인 스타일 체인 반복**도 중복이다.
- 판정 기준:

  | 상황 | 조치 |
  | --- | --- |
  | 같은 책임이 2개 파일에 존재 | 승격 후보. `PROJECTMAP.md`에 기록 |
  | 같은 책임이 3개 파일 이상 존재 | **즉시 승격** |
  | 동일 스타일 체인이 한 파일에서 3회 이상 반복 | 그 파일 안에서 헬퍼로 추출 |

### 승격 위치

| 반환/성격 | 위치 |
| --- | --- |
| GPUI 엘리먼트를 반환하는 UI 프리미티브 | `window/ui.rs` (배지·스탯 타일·액션 버튼·토글 행 등) |
| 스크롤/레이아웃 래퍼 | `window/mod.rs` (`scroll_pane`) |
| 엘리먼트를 반환하지 않는 순수 함수 | 주인이 있는 도메인 모듈(`config.rs`·`sync.rs`·`logging.rs`) 우선, 어디에도 속하지 않으면 `util.rs` |
| Win32 래퍼 | `platform/windows/mod.rs` (`wide_null` 등) |

- **패널 파일(`window/<기능>.rs`)에 범용 헬퍼를 남기지 않는다.** 그 파일 고유의
  도메인 지식이 들어간 헬퍼만 남긴다.
- 승격한 헬퍼는 색상을 인자로 받기보다 `cx.theme()`를 직접 읽는 형태를 기본으로 한다.
  호출부마다 토큰을 골라 넘기면 의미 매핑이 흩어진다.
- 승격 후 원본 정의는 **반드시 삭제한다.** 남겨 두고 새 것을 추가하면 중복이 늘어난다.

## 프로젝트 맵 관리 기준

`PROJECTMAP.md`는 저장소의 **구조·크기·공용 유틸을 추적하는 단일 문서**다.

- 소스 파일을 **추가·삭제·분할**했거나 **헬퍼를 공용으로 승격**했으면 같은 작업에서
  `PROJECTMAP.md`를 갱신한다.
- 줄 수는 아래 명령으로 실측한 값을 적는다. 추정하지 않는다.

  ```powershell
  (Get-ChildItem app/src -Recurse -Filter *.rs | ForEach-Object {
      [PSCustomObject]@{ File = $_.FullName; Lines = (Get-Content $_).Count }
  } | Sort-Object Lines -Descending)
  ```

- 각 파일에는 **한 줄 책임 설명**을 함께 적는다. 설명이 두 문장 이상 필요하면
  책임이 섞였다는 신호이므로 리팩터링을 검토한다.
- **공용 유틸 인벤토리**와 **중복 헬퍼 추적** 표를 유지한다. 새 헬퍼를 만들기 전에
  이 표를 먼저 확인해 이미 있는 것을 다시 만들지 않는다.
- `PROJECTMAP.md`, `TODO.md` 같은 현재 상태·대기열 문서에서 처리 완료 항목을 취소선으로
  남기지 않고 행이나 항목 자체를 삭제한다. 보존할 가치가 있는 완료 결과는 `MasterPlan.md`
  「구현 완료 단계」 또는 `PROJECTMAP.md`「리팩터링 이력」에 한 번만 기록한다.

## GPUI 및 테마 기준

- GPUI 0.2.2, gpui-component 0.5.1 호환 API만 사용한다.
- 색상은 반드시 `cx.theme().<토큰>` 방식으로 사용한다.
- 하드코딩 색상값을 사용하지 않는다.
- 의미 기반 토큰 매핑을 유지한다.
  - 페이지 배경: `background`
  - 기본 텍스트: `foreground`
  - 주요 액션: `primary`, `primary_foreground`
  - 보더: `border`
  - 사이드바: `sidebar`, `sidebar_foreground`, `sidebar_primary`, `sidebar_primary_foreground`, `sidebar_accent`
  - 카드 유사 표면: `secondary` 또는 `list`
  - 위험 상태: `danger`
- `card`, `destructive` 토큰에 의존하지 않는다.
- 스위치는 직접 `Switch::new`로 만들지 않고 `window::ui::toggle_switch`를 사용한다.
  번들·사용자 테마에서 `switch.*` 토큰이 빠지거나 표면색과 겹쳐도 트랙·썸·외곽선의
  비텍스트 대비가 유지되어야 한다.
- 테마 모드 변경은 `crate::theme::change_theme`를 사용한다. `Theme::change`를 직접 호출하면
  컴포넌트 팔레트 대비 보정이 누락된다.
- 번들 테마를 추가·교체하면 전체 테마 변형을 대상으로 스위치 트랙·썸·외곽선의 최소 대비
  테스트를 갱신하고 실행한다.

## UI 구성 기준

- `h_flex`, `v_flex` 중심으로 구성한다.
- 간격은 `gap` 계열 규칙을 우선 사용한다.
- 렌더 트리는 가능한 얕게 유지한다.
- 렌더 경로에 비즈니스 로직을 넣지 않는다.
- UI 코드에서 `unwrap` 사용을 지양한다.
- 사이드바 내비게이션은 그룹 컨테이너와 개별 항목 모두 `sidebar_border` 기반 경계를 두어,
  비활성 항목도 서로 구분되어야 한다.

### 스크롤 컨텐츠 높이 규칙

- 스플리터의 각 영역은 `window::scroll_pane`으로 감싸고, 표시 영역을 넘을 때 세로 스크롤과
  스크롤바가 동작하도록 한다.
- 스크롤 뷰포트와 그 상위 flex 체인은 `h_full`/`size_full`과 `min_h_0`로 높이를 제한한다.
- **스크롤되는 컨텐츠 루트는 자연 높이를 유지한다.** 컨텐츠 루트에 `size_full`·`h_full`,
  또는 높이를 먹는 `flex_1` + `min_h_0` 조합을 적용하면 스크롤 범위가 뷰포트 높이로
  고정되어 하단 내용이 잘릴 수 있으므로 사용하지 않는다. 너비만 채울 때는 `w_full`을 쓴다.
- 긴 목록·설정 폼은 최소 지원 창 높이에서 마지막 항목까지 스크롤되는지 확인한다.

## 상호작용 및 상태 기준

- 이벤트 처리는 listener 패턴을 사용한다.
- 로컬 상태 변경 후 필요한 경우 `cx.notify`를 호출한다.
- `div` 클릭 상호작용은 상태 기반 interactivity 요건을 만족한다(`.id()` 필요).
- 테마 변경은 `crate::theme::change_theme(ThemeMode::Light 또는 ThemeMode::Dark,
  Some(window), cx)`로 처리한다.

## 상태 전달 기준

- **UI → 백그라운드**는 공유 뮤텍스로 전달한다(`ScannerState`, `SyncSharedState`).
  상태를 바꾼 뒤 반드시 동기화 함수를 호출한다.
- **백그라운드 → UI**는 `PlatformEvent` 채널로만 전달한다.
  UI 이벤트 핸들러도 상태를 직접 고치지 말고 채널을 경유한다.
- 예외: 동기 호출이 필요한 서비스 제어(`service_mgr.rs`)는 platform 메서드를 직접 호출한다.

## 설정 저장 기준

- 설정 저장은 반드시 `config::update_config`를 사용한다.
  읽기-수정-쓰기를 한 경로로 모아 필드 유실을 방지하기 위한 규칙이다.
- 개별 저장 지점에서 `AppConfig`를 직접 구성하지 않는다.
- 새 필드에는 `#[serde(default)]`를 붙여 기존 config.json 호환성을 유지한다.

## 플랫폼 및 안전 기준

- 플랫폼 종속 코드는 `platform` 경로 하위에 분리한다.
- Windows 종속 구현은 `cfg(target_os = "windows")` 게이트를 유지한다.
- 사용자 요청 없는 파괴적 git 명령은 사용하지 않는다.
- 관련 없는 기존 변경사항은 보존한다.

## 라이브러리 문서 조회 기준

- GPUI, gpui-component, windows-sys 등 외부 라이브러리 API를 사용할 때는 반드시 mcp_context7을 통해 최신 문서를 조회한다.
- 조회 순서: `mcp_context7_resolve-library-id`로 라이브러리 ID 확인 → `mcp_context7_query-docs`로 특정 API 문서 조회.
- 라이브러리 버전이 명시된 경우 `/org/project/version` 형식 ID를 사용한다.
- 주요 라이브러리 참조:
  - gpui-component: `/longbridge/gpui-component` (현재 버전 0.5.1)
  - GPUI 프레임워크: gpui 관련 API는 gpui-component 문서에서 함께 조회
  - windows-sys: Windows Win32 API 사용 전 함수/상수 이름 확인
- mcp_context7에서 확인한 API 패턴이 실제 빌드 결과와 다를 경우, 로컬 크레이트 소스(`~/.cargo/registry`)를 직접 확인하여 최종 판단한다.
  context7 문서는 main 브랜치 기준이라 0.5.1과 다를 수 있으므로, 시그니처 확인은 로컬 소스가 우선이다.

## 검증 및 완료 보고 기준

- 코드 수정 후 `cargo check`를 수행한다.
- 단계 완료 확인 시 `cargo build`와 `cargo test`를 수행한다.
- 새로 유입된 오류는 종료 전에 해결하거나 원인을 명시한다.

## GPUI 자체 테스트 컨텍스트 필수 검증

GPUI 레이아웃, 색상·테마, 가시성, 스크롤·클리핑, 포커스 또는 사용자 상호작용을 변경하는
모든 작업은 GPUI 자체 테스트 컨텍스트를 사용한 회귀 테스트를 **반드시 추가하거나 갱신**한다.
헬퍼 함수만 검사하는 순수 단위 테스트나 Computer Use 캡처만으로 이 요구를 대신할 수 없다.

- `#[gpui::test]`와 `TestAppContext`/`VisualTestContext`로 실제 대상 뷰를 창에 렌더링한다.
- `app/Cargo.toml`의 dev-dependency에서 `gpui/test-support` feature를 유지한다. 일반
  dependency나 `--all-features`만으로 이 feature가 자동 활성화된다고 가정하지 않는다.
- 변경 수용 기준마다 테스트 이름, 사전 상태, 창 크기, 입력, 기대 상태를 일대일로 연결한다.
- 기본 창 크기와 변경 영역이 지원해야 할 최소 창 크기를 모두 검증한다. 최소 크기가 아직
  정해지지 않았다면 해당 UI 변경에서 명시하고 테스트로 고정한다.
- 크기 회귀는 `simulate_resize`, 클릭·키보드·wheel 입력은 `simulate_click`,
  `simulate_keystrokes`, `simulate_event` 등 현재 고정된 GPUI 버전의 API로 재현한다.
- 가능한 요소에는 안정적인 `debug_selector`를 부여하고 `debug_bounds`로 위치·크기·도달
  가능성을 검증한다. selector로 찾을 수 있는 요소를 취약한 절대 좌표에만 의존하지 않는다.
- 입력 뒤에는 엔티티·앱 상태, 선택·포커스 상태, 스크롤 핸들 offset, 대상 bounds 등
  수용 기준을 나타내는 값을 단언한다. 단순히 패닉 없이 렌더링됐다는 사실만으로 통과하지 않는다.
- UI가 테스트하기 어려우면 selector, 상태 조회 또는 테스트용 생성 경로를 같은 변경에
  포함한다. 테스트 seam이 없다는 이유로 GPUI 테스트를 생략하지 않는다.

영역별 최소 자동 검증은 다음과 같다.

- 사이드바: 기본·최소 창 크기에서 그룹과 항목의 bounds가 유지되고, 활성·비활성 상태에
  맞는 경계 토큰 정책을 별도 단언한다.
- 스위치·테마: light/dark와 변경 관련 테마에서 실제 스위치를 on/off 조작하고 상태 전이를
  단언한다. 팔레트 대비 순수 테스트는 전체 번들 테마를 검사하되 GPUI 렌더·입력 테스트를
  대체하지 않는다.
- 파일 동기화: 최소 창 높이에서 좌·우 패널을 각각 wheel 입력으로 스크롤하고 offset 변화와
  마지막 컨트롤의 viewport 진입을 단언한다.

관련 GPUI 테스트를 먼저 개별 실행한 뒤 단계 완료 전
`cargo test --all-targets --all-features`를 통과시킨다. 완료 보고에는 테스트 이름, 창 크기,
수행 입력, 기대·관찰 단언과 명령 결과를 기록한다. 관련 테스트가 없거나 실패하면 UI 작업은
완료가 아니며, 아래 실제 앱 시각 검증도 `PASS`로 판정할 수 없다. OS 통합처럼 테스트
컨텍스트가 직접 재현하지 못하는 범위는 한계를 명시하고 실제 앱에서 추가 검증하되, GPUI가
소유한 렌더·레이아웃·입력 범위의 테스트는 여전히 필수다.

## 실행 표면 하드 게이트

이 판정은 **모든 Computer Use 초기화와 실제 앱 실행보다 먼저** 수행한다. 플러그인 토글의
표시 여부가 아니라 현재 요청을 실행하는 제품 표면을 기준으로 한다.

| 현재 표면 | 판정 | 허용되는 검증 |
| --- | --- | --- |
| VS Code/Cursor의 Codex·Copilot 확장, 일반 터미널 | `IDE` | 정적 검사, Rust/GPUI 테스트, 데스크톱 인계 준비 |
| Windows에서 실행 중인 Claude Code | `CLAUDE_LOCAL` | IDE 검증 전부 + 로컬 하네스로 실제 화면 검증 |
| Windows ChatGPT 데스크톱 앱의 Work 또는 Codex | `DESKTOP` | 인계된 빌드의 Computer Use 실제 화면 검증 |
| 비Windows Claude Code, 제품 표면을 확정할 수 없음 | `IDE` | 안전하게 IDE 절차만 수행 |

`IDE`와 `DESKTOP`을 가르던 기존 규칙은 **ChatGPT 데스크톱의 Computer Use를 전제로** 만들어졌다.
VS Code의 Codex·Copilot 확장에서 그 wrapper·native pipe를 억지로 살리려다 오작동이 반복됐기
때문에 그 표면에서는 시도 자체를 금지한다. Claude Code는 애초에 그 API를 갖고 있지 않아
같은 오작동이 발생할 수 없고, 대신 저장소가 소유한 다른 경로가 있으므로 별도 표면으로 둔다.

### IDE 표면

- Computer Use 플러그인 토글이 보여도 지원 표면으로 간주하지 않는다.
- Computer Use 스킬·wrapper를 초기화하거나 `sky.*`를 호출하지 않는다. native helper·pipe
  확인, 재생성, 직접 실행, 확장·IDE 재시작도 시도하지 않는다.
- `.vscode/tasks.json`의 `GPUI: Verify in VS Code (no Computer Use)` 또는
  `scripts/Verify-Workspace.ps1`로 자동 검증을 수행한다.
- 자동 검증이 통과하면 구현 상태를 `IDE_VERIFIED`, 실제 화면 상태를 `DESKTOP_PENDING`으로
  보고한다. `DESKTOP_PENDING`은 정상 인계 상태이며 오류나 `BLOCKED`가 아니다.
- 실제 화면 검증이 필요하면 `GPUI: Prepare ChatGPT desktop handoff` 작업으로
  `target/visual-validation/handoff.json`과 해시 고정 바이너리를 생성한다. IDE 작업은 여기서
  종료하며 같은 요청에서 Computer Use를 재시도하지 않는다.
- 과거 `native pipe` 오류를 `MasterPlan.md`나 `TODO.md`에 반복 기록하지 않는다.

### CLAUDE_LOCAL 표면 (Windows Claude Code)

Claude Code에는 ChatGPT 데스크톱의 Computer Use(`sky.*`, `codex-computer-use.exe`, native pipe)에
해당하는 도구가 **아예 없다.** 그러므로 이 표면에는 "Computer Use 초기화 시도"나 "wrapper 우회"라는
개념 자체가 성립하지 않는다. 대신 저장소가 소유한 `scripts/Invoke-ClaudeVisualCheck.ps1`이
Win32 창 캡처·입력을 제공하고, Claude는 저장된 PNG를 Read 도구로 직접 관찰한다.

- 캡처는 `PrintWindow(..., PW_RENDERFULLCONTENT)`로 **대상 창만** 가져온다. 전체 데스크톱을
  캡처하지 않으므로 창이 가려져 있어도 되고, 사용자의 다른 화면 내용은 파일에 남지 않는다.
  GPU 렌더링(GPUI/Blade) 내용도 이 플래그가 있어야 비트맵에 들어온다.
- 입력은 `SendInput` 휠·좌클릭과 `MoveWindow` 크기 변경을 지원한다. 좌표는 클라이언트 영역
  기준 0~1 비율이라 창 크기가 달라져도 같은 지점을 가리킨다.
- 검증 대상 프로세스는 작업 전용 임시 루트를 `GPUI_CONVENIENCE_TOOLS_DATA_DIR`로 지정해
  실행한다. 앱은 `dirs::config_dir()`(= `SHGetKnownFolderPath`)로 데이터 루트를 찾으므로
  **`APPDATA` 환경 변수만 바꾸는 격리는 동작하지 않는다.**
- 검증이 끝나면 `-Action Stop`으로 기록된 PID와 작업 전용 임시 루트만 정리한다.

```powershell
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Start -Width 920 -Height 480
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name sidebar-before
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Wheel -X 0.12 -Y 0.65 -Delta -8
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name sidebar-after
scripts\Invoke-ClaudeVisualCheck.ps1 -Action Stop
```

이 표면의 한계는 그대로 보고한다. 한계를 넘는 수용 기준은 `PASS`로 판정하지 않는다.

- `SendInput`은 데스크톱 전역 입력이라 실제 커서가 움직이고 대상 창이 포그라운드여야 한다.
  하네스는 포그라운드 확보에 실패하면 입력을 보내지 않고 중단하지만, **사용자가 다른 작업을
  하는 중에는 실행하지 않는다.** 검증 전에 사용자에게 알린다.
- 대상 앱이 관리자 권한이고 Claude Code가 아니면 UIPI가 입력을 차단한다.
- 접근성 트리 조회가 없어 좌표는 기하학적으로 정한다. 요소 단위 단언은 여전히
  `debug_selector` 기반 GPUI 자체 테스트가 정본이고, 캡처는 그것을 대체하지 않는다.
- 캡처는 정지 화면이라 애니메이션·순간 상태는 잡지 못한다.
- 키보드 텍스트 입력은 하네스가 지원하지 않는다. 텍스트 입력이 필요한 수용 기준은
  GPUI 자체 테스트로 검증한다.

### DESKTOP 표면

- `target/visual-validation/handoff.json`의 `state`, 커밋·dirty 상태, 바이너리 경로와 SHA-256을
  먼저 확인한다. manifest가 없거나 해시가 다르면 실제 화면 검증을 시작하지 않고 IDE 인계
  준비를 요청한다.
- `scripts/Start-DesktopVisualValidation.ps1`로 manifest의 해시 고정 바이너리를 작업 전용
  임시 `APPDATA`에서 실행한다. 출력이 보이지 않아도
  `target/visual-validation/last-session.json`에서 PID·창 대상·격리 경로를 읽은 뒤
  Computer Use health check와 실제 화면 검증을 수행한다.
- 검증이 끝나면 `scripts/Stop-DesktopVisualValidation.ps1`로 기록된 PID와 작업 전용 임시
  루트만 정리한다.
- 이 표면에서만 `PASS`, `FAIL`, `BLOCKED`를 판정한다.

상태 의미는 다음과 같다.

- `IDE_VERIFIED`: 코드·GPUI 자동 검증 통과. IDE 구현 작업은 완료해 인계할 수 있다.
- `DESKTOP_PENDING`: 실제 화면 검증 대기. 시각·릴리즈 수용은 아직 `PASS`가 아니다.
  `IDE` 표면에서만 쓰는 정상 인계 상태다.
- `PASS` / `FAIL` / `BLOCKED`: `CLAUDE_LOCAL` 또는 `DESKTOP` 표면에서 실제 조작을 시도한
  결과에만 사용한다. `CLAUDE_LOCAL`에서 하네스가 커버하지 못하는 수용 기준이 남으면 그
  항목만 한계로 명시하고, 그 항목을 근거로 종합 `PASS`를 내지 않는다.

## GPUI 시각 검증 및 독립 크로스체크

GPUI 레이아웃, 색상·테마, 가시성, 스크롤·클리핑, 사용자 상호작용을 변경하면 정적 검사와
GPUI 자체 테스트만으로 완료하지 않는다. 위 필수 테스트가 통과한 뒤 다음 두 검증을
**순차적으로** 수행한다. 같은 데스크톱을 동시에 조작하면 상태와 증거가 섞일 수 있으므로
병렬 실행하지 않는다.

실제 화면 검증을 수행할 수 있는 표면은 `CLAUDE_LOCAL`과 `DESKTOP` 두 가지다.
`IDE`에서 구현했다면 `Verify-Workspace.ps1 -PrepareDesktopHandoff`가 만든 manifest와 해시 고정
바이너리, 수용 기준을 ChatGPT 데스크톱 검증 세션에 인계한다.
`CLAUDE_LOCAL`에서는 인계 없이 `scripts/Invoke-ClaudeVisualCheck.ps1`으로 두 검증을 모두
수행한다. ChatGPT 데스크톱 Computer Use(`sky.*`)를 쓰는 것은 `DESKTOP` 표면뿐이다.

1. 구현 담당자가 실제 빌드의 앱을 Computer Use로 직접 조작하고 수용 기준별 캡처를 남긴다.
2. 구현에 참여하지 않고 파일을 편집하지 않는 Visual Reviewer가 별도 검증 세션에서 같은
   수용 기준을 독립적으로 재현하고 자체 캡처를 남긴다.

- 구현 담당자의 캡처나 결론만 다시 읽는 것은 독립 크로스체크가 아니다.
- 조작·캡처에는 성공했지만 관찰이 하나라도 수용 기준과 다르면 `FAIL`이다.
- 필수 조작·캡처를 완료하지 못하면 `BLOCKED`다.
- 두 검증이 모두 통과해야 종합 `PASS`다. 종합 상태 우선순위는 `FAIL` > `BLOCKED` > `PASS`다.
  결과가 다르면 원인을 수정한 뒤 양쪽 검증을 다시 수행한다.
- 파괴 버튼, 실제 데이터 삭제, 의도하지 않은 파일 동기화는 시각 검증에서 실행하지 않는다.
- 별도 검증 세션은 검증자가 첫 캡처부터 직접 조작한다는 뜻이며 사용자 설정 초기화를 뜻하지
  않는다. `%APPDATA%` 설정을 삭제·교체하지 않는다. 파일 동기화 검증은 실행·삭제 버튼과
  옵션 값을 바꾸지 않고 탐색·스크롤만 수행한다.
- 검증할 앱 프로세스는 작업 전용 임시 루트 아래의 빈 디렉터리를 process-scoped
  `GPUI_CONVENIENCE_TOOLS_DATA_DIR`로 지정해 실행한다. **`APPDATA`만 바꾸면 격리되지 않는다** —
  앱은 `dirs::config_dir()`(= `SHGetKnownFolderPath`)로 데이터 루트를 찾으므로 그 환경 변수를
  읽지 않는다. 파일 동기화 검증에 필요한 원본·대상 폴더도 같은 임시 루트 아래에 만들고 그
  범위에서만 테스트 작업을 구성한다. 기존 앱 프로세스나 사용자 `%APPDATA%`를 재사용하지
  않는다. 격리 실행이 불가능하면 해당 시나리오는 `BLOCKED`로 보고한다.
- 격리가 실제로 걸렸는지는 캡처로 확인한다. 격리된 프로필은 기본 테마·빈 상태로 뜨므로,
  사용자의 저장된 설정이 보이면 격리가 깨진 것이다.

Codex는 Visual Reviewer를 생성할 때 `.github/agents/ui-visual-reviewer.agent.md`를 먼저
읽도록 위임한다. Claude Code는 `.claude/agents/ui-visual-reviewer.md`의 프로젝트
서브에이전트를 사용한다. 두 어댑터 모두 이 문서의 같은 계약을 적용한다.
현재 표면이 `IDE`이면 Visual Reviewer를 실행하지 않고 `DESKTOP_PENDING`으로 인계한다.
`CLAUDE_LOCAL`이면 Visual Reviewer가 로컬 하네스로 자체 세션을 열어 독립 검증을 수행한다.

### ChatGPT 데스크톱 Computer Use health check

- 먼저 「실행 표면 하드 게이트」가 `DESKTOP`인지 확인한다. `IDE`이면 wrapper를 초기화하지
  않고 `DESKTOP_PENDING`으로 종료한다.
- handoff manifest의 바이너리 SHA-256을 다시 계산해 일치하는지 확인한다.
- 매 새 `node_repl` 세션에서 설치된 Computer Use 스킬을 먼저 읽는다.
- 스킬이 제공하는 `<plugin-root>/scripts/computer-use-client.mjs`의
  `setupComputerUseRuntime`으로만 초기화하고 `sky.documentation("guidance")`를 읽는다.
- `sky.list_apps()` 또는 `sky.list_windows()`가 반환한 앱·창 객체 중 대상 창 하나를 명확히
  선택한 뒤 `sky.get_window_state({ window: targetWindow })`의 첫 캡처까지 성공해야 앱
  검증을 시작한다.
- `@oai/sky` 직접 import, `codex-computer-use.exe` 직접 실행, 사용자 정의 native-pipe
  클라이언트, PowerShell UI 자동화로의 우회는 금지한다. 이 금지는 **공식 Computer Use API가
  존재하는 `DESKTOP` 표면 한정**이다. 그 API가 없는 `CLAUDE_LOCAL`에서 저장소가 소유한
  `Invoke-ClaudeVisualCheck.ps1`을 쓰는 것은 우회가 아니라 유일한 정규 경로다.
- 초기화나 첫 캡처가 실패하면 새 `node_repl` 세션에서 공식 wrapper 경로로 한 번 재시도한다.
  지원되는 ChatGPT 데스크톱 표면에서도 네이티브 helper·pipe가 계속 없으면 데스크톱 앱을
  완전히 재시작한 다음 새 세션에서 다시 확인한다. 현재 세션에서 직접 helper를 띄우거나
  재시작을 성공한 검증으로 간주하지 않는다.

### 영역별 회귀 시나리오

변경한 영역 또는 관련 회귀 위험이 있는 영역의 시나리오만 적용한다.

- 사이드바 변경: 활성·비활성 그룹과 개별 항목의 경계가 서로 구분되는지 확인한다.
- 스위치·테마 변경: 기본 light/dark와 이슈가 보고된 테마에서 on/off 트랙·썸·외곽선이 모두
  보이는지 확인한다. 자동 대비 테스트는 전체 번들 테마 변형을 대상으로 한다. 사용자 테마는
  로드 시 런타임 팔레트 보정 후 해당 테마를 직접 시각 검증한다.
- 파일 동기화·스크롤 변경: 최소 지원 창 높이에서 좌·우 패널의 overflow 스크롤바가 나타나고,
  해당 패널 안에서 wheel 또는 drag로 마지막 항목까지 도달하는지 확인한다.

### 증거와 차단 보고

`CLAUDE_LOCAL`·`DESKTOP` 검증 결과에는 `Overall(PASS|FAIL|BLOCKED)`, 검증자 역할,
빌드/커밋·SHA-256, 도구·런타임,
시나리오별 사전 조건(테마·창 크기·앱 상태), 동작, 기대 결과, 관찰 결과, 캡처 식별자·시각,
결과, 검증 간 불일치, 잔여 위험을 기록한다.

`BLOCKED`에는 실패 단계(surface/import/setup/attach/capture/input), 실행한 정확한 API 또는
명령, 원문 오류, 복구 시도, 대체 정적·자동 검증 결과, ChatGPT 데스크톱 인계 필요 여부,
아직 확인하지 못한 수용 기준을 함께 적는다. `CLAUDE_LOCAL`에서는 캡처 PNG 경로를 증거
식별자로 쓰고, 하네스 한계로 확인하지 못한 항목을 분리해 적는다.
기존 스크린샷이나 구두 설명만으로 시각 검증을 통과 처리하지 않는다.

IDE 보고에는 `IDE_VERIFIED` 또는 자동 검증 실패, `DESKTOP_PENDING`, 실행한 VS Code
작업/명령, handoff manifest 경로만 기록한다. IDE 표면 자체는 `BLOCKED(surface)`로 보고하지
않으며 native pipe 오류를 만들기 위한 호출도 하지 않는다.

## 작업 후 오류 리뷰 기준

- 각 구현 작업 직후 Error Reviewer 서브 에이전트로 오류 전용 리뷰를 수행한다.
- 리뷰 범위는 컴파일, 린트, 진단 오류로 제한한다.
- 완료 보고에는 아래 중 하나를 반드시 포함한다.
  - 코드 오류 없음
  - 오류 목록, 위치, 원인 요약

## 문서 구조

- 메인 지침: `.github/copilot-instructions.md`
- 파일별 자동 적용 지침: `.github/instructions/gpui-core.instructions.md`
- 오류 분석 서브 에이전트: `.github/agents/error-reviewer.agent.md`
- 독립 시각 검증 서브 에이전트: `.github/agents/ui-visual-reviewer.agent.md`
- 루트 안내 문서: `AGENTS.md`, `CLAUDE.md`
- 계획과 완료 이력: `MasterPlan.md` / 미착수 대기열: `TODO.md`
- 구조·크기·공용 유틸 추적: `PROJECTMAP.md`
