---
name: gpui-rust-ui
description: Enforces strict Rust + GPUI UI generation using JSON-based theme tokens from gpui-component to produce modern, consistent UI.
version: 2.0
---

# ROLE
You are an expert Rust UI engineer using GPUI and gpui-component.

---

# OBJECTIVE
Generate modern, production-quality UI that strictly follows:
- JSON-based theme tokens (NOT simple dark/light)
- Clean layout system
- Rust safety rules

---

# THEME SYSTEM (CRITICAL)

## Core Principle
Theme is NOT dark/light toggle.

Theme is a structured JSON token system like:
- background
- foreground
- primary
- secondary
- muted
- accent
- border
- card
- destructive

ALL UI MUST be derived from these tokens.

---

## REQUIRED USAGE

ALWAYS use:
cx.theme().<token>

Examples:
- cx.theme().background
- cx.theme().foreground
- cx.theme().primary
- cx.theme().card
- cx.theme().border

---

## FORBIDDEN

NEVER use:
- hardcoded colors
- manual rgb/hex
- ad-hoc styling

INVALID:
"#ffffff"
Color::rgb(...)

---

## STATE VARIANTS (IMPORTANT)

Theme includes interactive variants.

ALWAYS use token-based state styling:

.hover(|s| s.bg(cx.theme().primary_hover))
.active(|s| s.bg(cx.theme().primary_active))
.disabled(|s| s.bg(cx.theme().muted))

---

## SEMANTIC USAGE (VERY IMPORTANT)

You MUST map UI meaning to tokens:

- Page background → background
- Text → foreground
- Primary action → primary
- Secondary UI → secondary
- Card surface → card
- Borders → border
- Disabled → muted
- Danger → destructive

DO NOT mix meanings arbitrarily.

---

# LAYOUT SYSTEM

## REQUIRED
- flex-based layout
- gap-based spacing

ALWAYS use:
.flex()
.flex_col()
.gap_*

---

## FORBIDDEN
- margin-based layout
- pixel magic numbers

---

# NESTING RULE

- MAX 3 levels deep
- Flatten aggressively

---

# COMPONENT STRUCTURE

Use structured composition:

- Root container
- Layout stack
- Content blocks
- Actions

Avoid meaningless div nesting.

---

# RUST SAFETY RULES

## Borrow
- NEVER create overlapping mutable borrows
- Extract reused values

## Clone
- NEVER clone unless required

## Option
- NEVER unwrap() in UI

---

# EVENT RULES

- Keep UI declarative
- Use event emission

.on_click(|cx| cx.emit(Event::Click))

---

# RENDER RULES

- NO business logic in render
- NO side effects

---

# MODERN UI RULES (KEY)

UI MUST look modern:

- Clear spacing hierarchy
- Minimal visual noise
- Strong contrast via theme tokens
- Consistent surface layering

---

# SURFACE SYSTEM (IMPORTANT)

Use layered surfaces:

- background → page
- card → content surface
- secondary/muted → sub sections

Example:

Page → background  
Card → card  
Subtle section → secondary  

---

# OUTPUT REQUIREMENTS

All generated UI MUST:

- Use ONLY theme tokens
- Respect semantic mapping
- Use flex + gap
- Avoid deep nesting
- Avoid unwrap / clone misuse
- Follow builder pattern

---

# FAILURE CONDITIONS

INVALID if:

- hardcoded colors exist
- theme tokens not used
- semantic mismatch (e.g. primary used as background)
- deep nesting (>3)
- layout without flex/gap

---

# REFERENCE EXAMPLE

fn render(cx: &mut ViewContext<Self>) -> impl IntoElement {
    div()
        .size_full()
        .bg(cx.theme().background)
        .child(
            div()
                .flex_col()
                .gap_4()
                .p_4()
                .child(
                    div()
                        .bg(cx.theme().card)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_md()
                        .p_4()
                        .child(
                            div()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_lg()
                                        .text_color(cx.theme().foreground)
                                        .child("Dashboard")
                                )
                                .child(
                                    button("Continue")
                                        .bg(cx.theme().primary)
                                        .text_color(cx.theme().primary_foreground)
                                        .hover(|s| s.bg(cx.theme().primary_hover))
                                )
                        )
                )
        )
}

---

# WINDOWS BUILD: MANIFEST 충돌 문제 (CRITICAL)

## 증상
릴리즈 빌드 시 다음 링커 오류 발생:
```
CVTRES : fatal error CVT1100: 중복 리소스. 유형:MANIFEST, 이름:1
LINK : fatal error LNK1123: COFF로 변환하는 중 오류가 발생했습니다.
```

## 원인
`gpui-component`가 `gpui`를 `default-features`로 의존하므로, 워크스페이스에서
`gpui`의 `default-features = false`를 지정해도 `windows-manifest` 피처가
`gpui-component`를 통해 항상 재활성화된다.

- gpui (windows-manifest 피처) → `.res` 파일에 `RT_MANIFEST ID=1` 임베드
- 자체 `build.rs`에서 `winres` 또는 `embed-resource`로 추가 매니페스트 임베드
- 링커 기본값 `/MANIFEST:EMBED` → 세 번째 `RT_MANIFEST` 생성 시도
→ CVTRES가 중복 감지 → LNK1123

## 해결 (검증된 방법)
`build.rs`에서 자체 매니페스트 임베딩을 제거하고 `/MANIFEST:NO` 링커 인자만 추가한다.

```rust
// build.rs
fn main() {
    #[cfg(target_os = "windows")]
    {
        // 링커 자동 매니페스트 생성 억제 (gpui 임베드 매니페스트와 중복 방지)
        println!("cargo:rustc-link-arg=/MANIFEST:NO");
    }
}
```

`Cargo.toml` build-dependencies에서 `winres`, `embed-resource` 제거:
```toml
# 제거 대상
[target.'cfg(target_os = "windows")'.build-dependencies]
# winres = "0.1"       ← 제거
# embed-resource = "2" ← 제거
```

결과: gpui 내장 매니페스트(DPI PerMonitorV2, Common Controls 6.0)만 EXE에 임베드됨.

## 주의사항
- `gpui = { version = "...", default-features = false }` 워크스페이스 설정은
  gpui-component가 `default-features`로 gpui를 사용하기 때문에 효과 없음.
- UAC `requireAdministrator` 매니페스트는 이 방법으로는 적용 불가.
  UAC 대신 `windows_subsystem = "windows"` 속성으로 GUI 앱으로 분류하는 방법을 사용.

---

# WINDOWS BUILD: 터미널 종료 시 트레이 앱 소멸 문제

## 증상
릴리즈 EXE를 터미널(PowerShell/CMD)에서 실행 후 터미널을 닫으면
트레이 아이콘이 사라지고 앱이 종료된다.

## 원인
`windows_subsystem = "windows"` 속성이 없으면 EXE가 **콘솔 서브시스템**으로
빌드된다. 콘솔 앱은 부모 터미널에 연결되며, 터미널 닫힘 시
`CTRL_CLOSE_EVENT`가 모든 연결 프로세스에 전파되어 앱이 종료된다.

## 해결
`src/main.rs` 최상단에 릴리즈 빌드 전용 속성 추가:

```rust
// 릴리즈 빌드: GUI 서브시스템 → 터미널 닫아도 CTRL_CLOSE_EVENT 전달 안 됨
// 디버그 빌드: 콘솔 연결 유지 → 로거 콘솔 출력 정상
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
```

## 주의사항
- `#![windows_subsystem = "windows"]`를 조건 없이 적용하면 디버그 빌드에서
  로거의 콘솔 출력이 불가능해진다. 반드시 `not(debug_assertions)`와 조합.
- 이 속성을 적용하면 해당 EXE는 콘솔 창 없이 실행된다.
  이 저장소는 `src/logging.rs`의 롤링 파일 로거가 항상 파일에도 기록하므로
  릴리즈 빌드에서도 `%APPDATA%\gpui-convenience-tools\logs\`에서 로그를 확인할 수 있다.