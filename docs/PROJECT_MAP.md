# PROJECT MAP — 구조 · 크기 · 공용 유틸 추적

> 이 문서는 저장소의 **구조·크기·공용 유틸을 추적하는 단일 문서**다.
> 소스 파일을 추가·삭제·분할했거나 **헬퍼를 공용으로 승격했으면 같은 작업에서 이 문서를 갱신한다.**
> 규칙 정의는 `DEVELOPMENT_GUIDE.md`의 「구조 리팩터링 기준(1,000줄 트리거)」과
> 「공용 유틸 승격 기준」 절을 정본으로 한다.
>
> **새 헬퍼를 만들기 전에 「공용 유틸 인벤토리」를 먼저 확인한다.**

## 구조 — VirtualBox 오프라인 디스크 탐색

구현된 파일과 예정 경로를 함께 추적한다. 새 파일을 추가하는 같은 작업에서 줄 수와 책임을
실측해 이 문서의 현재 파일 목록으로 승격한다.

| 경로 | 책임 | 관련 작업 ID |
| --- | --- | --- |
| `app/src/virtual_disk/mod.rs` | **구현됨** — `GuestFileSource`·파일 항목·오류 타입·백엔드 경계 | VDE-003 |
| `app/src/virtual_disk/vdi.rs` | **구현됨** — VDI 1.1 read-only 블록 리더와 잠금·VM 사용·원본 안정성 가드 | VDE-004~005 |
| `app/src/virtual_disk/partition.rs` | **구현됨** — MBR·EBR·protective MBR·GPT 검색과 CRC·범위 검증 | VDE-006 |
| `app/src/virtual_disk/ntfs.rs` | **구현됨** — NTFS 디렉터리 열거·기본 데이터 스트림 읽기·속성 보존·항목별 오류 경계 | VDE-007~008 |
| `app/src/virtual_disk/path_policy.rs` | **구현됨** — 게스트 경로의 호스트 매핑, 예약 이름·길이·링크 추적 안전 정책 | VDE-009 |
| `app/src/virtual_disk/copy.rs` | **구현됨** — 선택 파일·폴더의 청크 복사와 건너뜀·덮어쓰기·새 이름 충돌 정책, 메타데이터 결과 집계 | VDE-010~011 |
| `app/src/virtual_disk/issues.rs` | **구현됨** — 원본 변경·대상 쓰기·지원 불가·메타데이터 오류 분류, 부분 복사 결과와 항목별 억제 키 | VDE-012 |
| `app/src/virtual_disk/metadata.rs` | **구현됨** — 호스트 파일 속성·타임스탬프 적용과 구조화된 적용 실패 결과 | VDE-011 |
| `app/src/window/virtual_disk.rs` | VDI 선택·파티션·탐색·파일 행 선택·상위 이동·단축키·진행 UI | VDE-013~017 |
| `app/src/app/virtual_disk_ops.rs` | **구현됨** — read-only VDI 열기·파티션 검색/선택·게스트 경로 새로고침·폴더 이동·다중 선택·탐색기 포커스·실행 중/미지원 오류 메시지 | VDE-013~014·016~017 |
| `app/src/app/virtual_disk_copy.rs` | **구현됨** — 대상 폴더 입력/선택, read-only VDI 재연결 백그라운드 복사, 진행·중지·완료 요약 이벤트, 탐색기 keymap 등록 | VDE-015~016; 실제 이미지 E2E는 VDE-019 |
| `docs/VIRTUAL_DISK_BACKENDS.md` | **설계 정본** — 오프라인 VDI 기본 경로와 실행 중 VM `guestcontrol` 후속 백엔드의 분리·명령·안전·검증 경계 | VDE-021 |

안전 경계: 원본 VDI는 read-only로만 열고, 실행 중 VM의 VDI 직접 읽기는 구현하지 않는다.
실행 중 VM 지원은 후속 `GuestFileSource` 구현으로만 추가한다(VDE-021).

**최종 측정**: 2026-09-22 · `app/src` 총 53개 파일 · 20,883줄

## 크기 기준 — 줄 수는 증상이다

1,000줄 초과는 "파일이 길다"가 아니라 **책임 배치가 프로젝트 규모를 못 따라왔다**는 신호다.
따라서 조치는 파일 자르기가 아니라 **중복 제거 → 오배치 책임 이동 → (그래도 크면) 책임 단위 분할**
순서의 구조 리팩터링이다. ①~②만으로 임계값 아래로 내려가면 분할은 하지 않는다.

| 줄 수 | 상태 | 조치 |
| --- | --- | --- |
| ~800 | 🟢 정상 | — |
| 800~1,000 | 🟡 경고 | 다음 작업 전에 구조 리팩터링 |
| 1,000 초과 | 🔴 위반 | **즉시 리팩터링.** 다른 작업보다 우선 |

현재 🔴 위반 **없음**, 🟡 경고 **3개**. 최대 파일은 966줄(`virtual_disk/vdi.rs`)이며, 다음 VirtualBox UI 작업 전에 책임 단위 분할을 검토한다.
줄 수와 무관하게 처리하는 중복 헬퍼는 아래 「중복 헬퍼 추적」에서 관리한다.

### 줄 수 측정 명령

```powershell
Get-ChildItem app/src -Recurse -Filter *.rs | ForEach-Object {
    [PSCustomObject]@{ Lines = (Get-Content $_).Count; File = $_.FullName }
} | Sort-Object Lines -Descending
```

```bash
wc -l $(find app/src -name '*.rs' | sort) | sort -rn
```

---

## 파일 목록

### 루트 (`app/src/`)

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `main.rs` | 133 | 진입점 — 로거 설치 → 테마 시드 → 윈도우 오픈, `--service`/`--tray` 플래그 분기 |
| `theme.rs` | 143 | 테마 모드 적용과 스위치 팔레트 최소 대비 보정·번들 테마 감사 테스트 |
| `config.rs` | 751 | `AppConfig`·`SyncJob`·`WatchMode`·`SymlinkMode`·`LogConfig`·주기 프리셋·사이드바 폭·VDI 오류 억제 키 정의, 동기화 제외 패턴 스키마, `update_config` 단일 저장 경로, 데이터 루트 오버라이드 |
| `logging.rs` | 560 | 롤링 파일 로거 (`log::Log` 구현, 테스트용 출력 경로 주입) |
| `sync_history.rs` | 223 | 동기화 완료 이력의 JSON 배열 저장·순서 보장·손상 파일 보존·개수/기간 보존·최신 목록 로드·소요 시간 계산 |
| `util.rs` | 139 | 도메인 주인이 없는 순수 헬퍼 — `format_interval`·`interval_to_secs`·`TimeUnit` |

### VirtualBox 도메인 (`app/src/virtual_disk/`) — 4,732줄 / 8파일

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 456 | VDI·파티션·게스트 파일 항목 모델, 정규화된 게스트 경로·시간 메타데이터, `GuestFileSource`, 플랫폼 비의존 오류 계약 |
| `vdi.rs` | 966 | 표준 VDI 1.1 read-only 헤더·블록 맵·동적/고정 블록 읽기, 잠금·VM 사용·크기/mtime 안정성 가드, NTFS·미지원 파티션 VDI fixture export와 파서 경계 테스트 |
| `partition.rs` | 791 | read-only `PartitionSource` 경계, MBR·EBR·protective MBR·GPT 검색, 양쪽 CRC·LBA 범위 검증, NTFS 3.1 부트 섹터 판정 |
| `ntfs.rs` | 801 | 파티션 범위 `Read + Seek` 어댑터, NTFS 3.1 디렉터리·기본 데이터 스트림 읽기, 압축·암호화·범위·손상 오류와 속성·시간 보존, 고정/손상 픽스처 테스트 |
| `path_policy.rs` | 335 | 절대 대상 루트 기준 게스트 경로 매핑, Windows 예약 이름·경로 길이 검증, 게스트·호스트 리파스 포인트/심볼릭 링크 추적 차단 |
| `copy.rs` | 834 | `GuestFileSource` 선택 파일·폴더의 설정 청크 복사, 대상 부모 생성, 건너뜀·덮어쓰기·새 이름 충돌 정책과 메타데이터 결과 집계 |
| `issues.rs` | 146 | 복사 오류 종류·안정 억제 키·부분 복사 보고서 연결과 항목별 알림 억제 상태 |
| `metadata.rs` | 403 | Windows 파일 속성·생성/접근/수정 시간과 비지원·권한 오류를 `MetadataFailure`로 수집 |

### 동기화 엔진 (`app/src/sync/`) — 1,349줄 / 2파일

동기화 엔진을 `sync/mod.rs` 본문과 `sync/tests.rs` 테스트로 나눈 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 673 | 폴더 동기화 엔진 (UI 비의존 순수 로직) — 진행 보고·중지·이어서 시작·제외 glob·심볼릭 링크 모드 경계 포함 |
| `tests.rs` | 676 | 복사·건너뜀·심볼릭 링크 안전 건너뜀·미구현 모드 실패 경계·읽기 전용 대상 덮어쓰기·미러 삭제·실패 사유·진행 보고·중지·이어서 시작·제외 glob 단위 테스트 |

### 앱 루트 (`app/src/app/`) — 4,336줄 / 12파일

`app.rs`(1,798줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 790 | `AppRoot` 정의·생성자·검증 전용 초기 패널·VDI 시드 초기화·동기화 이력 초기 로드·백그라운드 UI wake·사이드바(전역 스위치 2개 포함)·폭/VDI 억제 설정 복원·최상위 레이아웃 |
| `sync_ops.rs` | 457 | 파일 동기화 작업 조작 (추가·삭제·선택·이름·경로·제외 패턴 입력 저장·수동 실행 큐·중지·전역 스위치·커서 무효화) |
| `interval.rs` | 313 | 주기 선택 상태(`IntervalPicker`)와 조작 — 프리셋 추가·삭제·드롭다운 동기화 |
| `events.rs` | 326 | `PlatformEvent` 채널 소비, 진행 상태·동기화 이력·감시 실패 강등 반영, 로그·토스트 유틸 |
| `background.rs` | 459 | 스캔 스레드와 동기화 스레드 (다중 광고 창 추적·복원·진행 이벤트 빈도 제한·중지·실행 위치·동기화 이력 영속화·실시간 watcher 연동) |
| `state.rs` | 262 | 순수 데이터 타입 (`AppState`, `PlatformEvent`, `SyncRunning`, `ActivePanel`, 감시 실패 강등 이벤트, 타깃별 `NAV_*`) |
| `ops.rs` | 195 | 광고 차단·서비스 관리·로그 설정 조작 |
| `inputs.rs` | 136 | 입력 위젯(`InputState`) 지연 생성과 값 동기화 — 파일 동기화 제외 패턴 멀티라인 편집기 포함 |
| `ui_state.rs` | 79 | `ServiceState`·`SyncState`·`AdBlockState`와 최근 동기화 이력 — 단일 `AppRoot` 엔티티가 소유하는 기능별 UI 상태 묶음 |
| `virtual_disk_ops.rs` | 524 | VDI 경로 입력·read-only 열기·파티션 검색/선택·게스트 목록 새로고침·폴더 이동·다중 선택·포커스·안전 오류 안내·항목별 반복 알림 억제 저장·검증 시드 상태 |
| `virtual_disk_copy.rs` | 492 | VDI 대상 폴더 입력·선택, 백그라운드 read-only 복사, 진행·중지·완료 요약·항목별 오류 로그·미억제 토스트 게이트·탐색기 keymap·검증 자동 복사 |
| `watch.rs` | 303 | `notify` 재귀 watcher 소유·작업별 변경 이벤트 전달·2초 quiet debounce·감시 실패 중복 억제·작업 변경 시 정리 |

### GPUI 회귀 테스트 (`app/src/app/tests/`) — 1,981줄 / 6파일

테스트는 시나리오별 6개 파일로 나뉘며, 픽스처는 `mod.rs`가 단독 소유하고 하위 모듈은
`use super::*`로 가져다 쓴다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `file_sync.rs` | 794 | 동기화 조작·진행 표시줄·중지·로그 요약·최근 이력 카드·섹션 너비·전역 스위치·커서 무효화·제외 패턴·감시 방식 UI 경계·저장 연결 |
| `layout.rs` | 397 | 사이드바·스플리터·카드 경계·divider drag·스크롤 |
| `interval.rs` | 226 | 주기 드롭다운·프리셋 추가/삭제·패널 간 공유 |
| `mod.rs` | 191 | 공용 픽스처 (`test_app_root`·`TestPlatform`·`refresh`·`click_debug_element` 등) |
| `theme.rs` | 85 | 테마 전환과 스위치 가시성 |
| `virtual_disk.rs` | 288 | VirtualBox 탐색 네비게이션·VDI 입력·파티션 카드·현재 경로·상위 이동·실제 목록·숨김/시스템 전체 선택·복사 진행/실패 요약·오류 억제 버튼 dispatch·키보드 단축키·안전/미지원 상태·새로고침 렌더 경계 |

### 패널 (`app/src/window/`) — 4,146줄 / 11파일

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `service_mgr.rs` | 670 | 편의 기능 — Windows 서비스 (목록/제어 ↔ 검색·필터·권한) |
| `file_sync.rs` | 727 | 편의 기능 — 파일 동기화 (작업 목록 → 설정·제외 패턴·감시 방식 → 실패 기록·최근 실행 이력 + 하단 고정 진행 표시줄) |
| `settings.rs` | 444 | 전역 설정 — 테마 선택·로그 보관 정책 |
| `ad_block.rs` | 502 | 편의 기능 — 웹뷰 광고 차단 (상태·타겟 ↔ 스캔 주기·프로세스 추가·카드 경계) |
| `service_view.rs` | 317 | 시스템 — 자동 시작(작업 스케줄러) 등록·삭제·즉시 실행 |
| `ui.rs` | 327 | **공용 UI 프리미티브** — 배지·액션 버튼·토글 스위치·통계 타일·설정 행·선택 칩·로그 레벨 칸·폭 경계 |
| `dashboard.rs` | 161 | 개요 — 전체 상태 요약과 최근 활동 (플랫폼별 요약 카드 + 동기화 상태 배지) |
| `interval.rs` | 154 | 주기 선택 렌더 — 드롭다운 + (값·단위·추가) 행 + 등록된 프리셋 목록 |
| `log_view.rs` | 110 | 시스템 — 화면 로그 가상 리스트와 로그 파일 현황 |
| `mod.rs` | 107 | 패널 모듈 선언 + `balanced_split`·`scroll_pane` 레이아웃 헬퍼 |
| `virtual_disk.rs` | 627 | 편의 기능 — VDI 경로 입력·파티션 선택·게스트 현재 경로·행 선택·폴더/상위 이동·단축키 포커스·안전/미지원 상태·대상 폴더·복사 진행·오류 요약 억제/재표시·목록 새로고침·검증 자동 스크롤 |

### 플랫폼 (`app/src/platform/`) — 2,390줄 / 8파일

`windows.rs`(1,361줄)를 책임별로 분할한 결과다.

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `mod.rs` | 220 | `Platform` trait 정의 + 광고 창 후보 목록·상태 스냅샷·서비스 타입, `NativePlatform` 타깃별 별칭 |
| `fallback.rs` | 59 | 비Windows `Platform` 구현 — 광고 차단 계열 미지원을 명시적으로 반환 |
| `windows/scm.rs` | 449 | Windows 서비스(SCM) 등록과 서비스 모드 실행 |
| `windows/services.rs` | 340 | 설치된 Win32 서비스 조회·시작·중지·삭제, 권한 확인 |
| `windows/tray.rs` | 274 | 시스템 트레이 아이콘과 메시지 루프 |
| `windows/window_ops.rs` | 719 | 타겟 프로세스 트리·최상위/명시적 자식 창 열거, 클래스 필터, 광고 팝업 후보 탐색, 창 상태 캡처·0×0 축소·복원 |
| `windows/mod.rs` | 178 | `WindowsPlatform` + `Platform` 구현, 다중 창 후보·상태 API 연결, 하위 모듈 re-export |
| `windows/task_scheduler.rs` | 148 | 로그온 시 자동 시작(`schtasks`) |

### 빌드

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `app/build.rs` | 12 | `/MANIFEST:NO` 링커 인자 (gpui 임베드 매니페스트 중복 방지) |

### 테스트 픽스처

| 경로 | 크기 | 책임 |
| --- | ---: | --- |
| `app/testdata/ntfs-testfs1.img` | 2 MiB | `ntfs` 0.4.0 배포본에서 고정한 NTFS 3.1 read-only 탐색 원본 |
| `docs/TEST_FIXTURES.md` | — | 고정 이미지 출처·임시 VDI 생성·손상 복사본·격리 경로 계약 |

### 개발·검증 도구

| 파일 | 줄 | 책임 |
| --- | ---: | --- |
| `.github/agents/code-reviewer.agent.md` | 70 | 공통 Code Reviewer 역할 정본 — 읽기 전용 종합 검토·검증 증거·문서 정합성 |
| `.github/agents/docs-sync.agent.md` | 69 | 공통 Documentation Sync 역할 정본 — `docs/` 정본 동기화·작업 ID·구조 지도 |
| `.claude/agents/code-reviewer.md` | 12 | Claude용 Code Reviewer 얇은 어댑터 |
| `.claude/agents/docs-sync.md` | 11 | Claude용 Documentation Sync 얇은 어댑터 |
| `.codex/agents/code-reviewer.toml` | 9 | Codex용 Code Reviewer 얇은 어댑터 |
| `.codex/agents/docs-sync.toml` | 9 | Codex용 Documentation Sync 얇은 어댑터 |
| `scripts/Verify-Workspace.ps1` | 167 | VS Code용 Rust/GPUI 자동 검증과 ChatGPT 데스크톱 handoff manifest·해시 고정 빌드 생성 |
| `scripts/Invoke-ClaudeVisualCheck.ps1` | 474 | `CLAUDE_LOCAL` 시각 검증 하네스 — 격리 실행(`-SeedConfig`·`-SeedHistory`·`-InitialPanel`로 상태 재현)·창 캡처(`PrintWindow`)·입력(`SendInput`)·정리 |
| `scripts/Verify-AdWindowState.ps1` | 282 | 지정 PID와 앱 조상·자손의 최상위·선택적 자식 창 상태와 클래스 후보를 읽기 전용 점검(AD-002·AD-005 진단) |
| `scripts/Start-DesktopVisualValidation.ps1` | 126 | manifest 해시 검증 후 단일 임시 데이터 루트 격리 프로세스·세션 파일 생성과 실패 롤백 |
| `scripts/Stop-DesktopVisualValidation.ps1` | 75 | 기록된 검증 PID·시작 시각과 작업 전용 임시 루트만 검증 후 정리 |
| `.vscode/tasks.json` | 111 | IDE 전용 검증, Claude 로컬 시각 세션, ChatGPT 데스크톱 인계 준비 작업 |
| `installer/macos/build-app.sh` | 135 | macOS 유니버설 `.app` 번들·ad-hoc 서명·DMG 생성 (macOS에서만 실행) |
| `.github/workflows/release.yml` | 140 | `v*` 태그 → Windows·macOS 병렬 빌드 후 단일 Release 생성 |
| `.github/workflows/macos-build.yml` | 47 | push/PR마다 macOS check·test·패키징 — 비Windows cfg 경로의 **유일한** 검증 지점 |

> `Invoke-ClaudeVisualCheck.ps1`은 474줄이지만 분할하지 않는다. 하나의 Win32 시퀀스
> (P/Invoke 선언 → 세션 → 캡처 → 입력 → 정리)를 공유하고, 쪼개면 각 파일이 같은 `Add-Type`
> 블록과 세션 스키마를 중복 소유하게 되어 응집도가 깨진다.
> 정본의 「구조 리팩터링 기준 > 예외」 조항을 적용한다.

---

## 공용 유틸 인벤토리

**새 헬퍼를 만들기 전에 이 표를 먼저 본다.** 여기에 있는 것을 각 패널에서 다시 만들지 않는다.

| 소유 위치 | 항목 | 용도 |
| --- | --- | --- |
| `window/ui.rs` | `badge(label, Tone, Size, cx)` | 상태 배지. 폭 고정은 반환값에 `.w(px(..))` |
| `window/ui.rs` | `action_button(id, label, Size, ButtonStyle, on_click)` | 클릭 가능한 액션 버튼 |
| `window/ui.rs` | `toggle_switch(id, checked, cx)` | 테마와 상태에 맞는 대비 외곽선을 갖는 공용 스위치 |
| `window/ui.rs` | `stat_tile(label, value, cx)` | 숫자 하나를 강조하는 통계 타일(`flex_1` 포함) |
| `window/ui.rs` | `option_row(id, title, description, checked, on_click, cx)` | 제목·설명 + 토글 스위치 설정 행. `{id}-row`/`{id}` debug_selector 부여 |
| `window/ui.rs` | `choice_chip(id, label, selected, cx)` | 프리셋 선택 칩. 호출부가 `.on_click(cx.listener(..))`를 이어 붙인다 |
| `window/ui.rs` | `log_level_label(level, cx)` | 로그 한 줄의 레벨 칸. 레벨 → 색 매핑 포함, 폭 고정 + 줄바꿈 금지 |
| `window/ui.rs` | `Tone` | 배지 의미 색 — `Success`·`Warning`·`Info`·`Muted` |
| `window/ui.rs` | `ButtonStyle` | 버튼 의미 색과 기본 hover/border 정책 — `primary`·`neutral`·`secondary`·`danger`·`danger_outline`·`muted` |
| `window/ui.rs` | `Size` | 여백 — `Sm`(px_2 py_1) · `Md`(px_3 py_1) · `Lg`(px_4 py_2) |
| `window/mod.rs` | `balanced_split(id, left_min, right_min, left, right)` | 양쪽 최소 폭을 보장하며 가용 너비를 균형 있게 채우는 공용 스플리터 |
| `window/mod.rs` | `scroll_pane(id, handle, content)` | 자연 높이 컨텐츠가 넘칠 때 세로 스크롤 + 스크롤바 부여. **가로는 잠근다**(`overflow_x_hidden`) |
| `config.rs` | `carry_over_engine_progress(stored, jobs)` | UI 스냅샷 저장 시 엔진이 기록한 `last_run_unix`·`resume_cursor`를 디스크에서 되살린다 |
| `sync/mod.rs` | `SyncControl::resume_from(path)` | 끊긴 순회를 그 상대 경로부터 이어서 시작 (앞 구간만 건너뜀) |
| `theme.rs` | `change_theme` · `normalize_component_palette` | 테마 변경 후 스위치 트랙·썸 최소 대비 보정 |
| `config.rs` | `update_config(edit)` | 설정 읽기-수정-쓰기 **단일 경로** |
| `config.rs` | `data_dir` · `config_path` · `themes_path` · `logs_path` | 데이터 루트 하위 경로 계산 (`GPUI_CONVENIENCE_TOOLS_DATA_DIR`로 재지정 가능) |
| `config.rs` | `default_interval_presets` · `normalize_interval_presets` | 주기 프리셋 기본값(10·30·60초)과 정규화(중복 제거·오름차순) |
| `util.rs` | `format_interval(secs)` | 초 → `10초`·`1분`·`1분 30초`·`1시간` 표기. **모든 주기 표시는 이것만 쓴다** |
| `util.rs` | `interval_to_secs(amount, unit)` · `TimeUnit` | 사용자 입력 (값, 단위) → 초. 범위 밖이면 보여줄 사유를 반환 |
| `sync.rs` | `SyncControl` · `SyncProgress` | 동기화 엔진의 진행 보고·중지 제어 (보고 빈도 제한은 호출자 몫) |
| `app/state.rs` | `SyncRunning::counters` · `display_path` | 진행 표시줄 문자열 — 경로는 앞을 줄이고 파일명을 남긴다 |
| `logging.rs` | `now_hms` · `current_log_file` · `log_dir_stats` | 시각 문자열, 로그 파일 현황 조회 |
| `platform/windows/mod.rs` | `wide_null(&str)` | Win32 UTF-16 널 종단 문자열 변환 |

### 승격 위치 규칙

| 성격 | 소유 모듈 |
| --- | --- |
| GPUI 엘리먼트를 반환하는 UI 프리미티브 | `window/ui.rs` |
| 스크롤/레이아웃 래퍼 | `window/mod.rs` |
| 엘리먼트를 반환하지 않는 순수 함수 | 도메인 모듈(`config`·`sync`·`logging`) 우선, 없으면 `util.rs` *(아직 없음)* |
| Win32 래퍼 | `platform/windows/mod.rs` |

패널 파일(`window/<기능>.rs`)에는 **그 기능 고유의 도메인 지식이 든 헬퍼만** 남긴다.

---

## 중복 헬퍼 추적

판정: 2개 파일 = 후보, **3개 파일 이상 = 즉시 승격**(발견한 작업에서 바로 처리),
한 파일 내 동일 체인 3회 이상 = 그 파일 안에서 추출.

| 중복 | 위치 | 판정 | 상태 |
| --- | --- | --- | --- |
| 필터 행 | `service_mgr.rs` 상태 필터(`px_3 py_2` + 개수 배지 2단 구성) | 🟢 1곳 | 칩과 형태가 달라 승격 대상 아님. 배지가 붙는 변형이 하나 더 생기면 재검토 |

현재 추적 중인 **승격 후보 없음**. 선택 칩 패딩 차이는 아래 「리팩터링 이력」의
2026-07-30 주기 UI 작업에서 `ui::choice_chip`으로 통일하며 해소했다.

**해소됨** — `ui::stat_tile`·`ui::option_row`·`ui::choice_chip`으로 승격하며 원본 정의를 모두
삭제했다. `ad_block.rs stat_card`(색 인자 6개)와 `dashboard.rs stat_tile`은 색 전달 방식만
달랐고, 토글 행은 `file_sync`·`settings`·`ad_block` **3개 파일**(즉시 승격 기준), 프리셋 칩은
`ad_block`·`file_sync`·`settings` **3개 파일**에 같은 스타일 체인이 복제돼 있었다.

**「초 단위 간격 표기」 판정 정정 (2026-07-30)** — 한때 "합치면 화면이 바뀌므로 승격 대상이
아니다"라고 적었으나, 이는 잘못된 기준이었다. **같은 값을 화면마다 다르게 찍는 것 자체가
결함**이므로 화면 변경은 보류 사유가 되지 않는다. `util::format_interval`로 통일했고,
이 판단 기준은 정본의 「구현 기준」 첫 항목에 규칙으로 올렸다.

**로그 레벨 칸** — 대시보드 64px·로그 패널 72px로 갈려 있었고 좁은 쪽에서 `SUCCESS`가 두 줄로
접혀 잘렸다. 레벨 → 색 매핑까지 두 곳에 복제돼 있어 `ui::log_level_label`로 승격했다(2곳이지만
한쪽이 실제로 깨져 있어 즉시 처리).

**이름이 다르다고 다른 헬퍼가 아니다.** `badge`/`state_badge`, `stat_card`/`stat_tile`처럼
한쪽은 색을 인자로 받고 한쪽은 `cx.theme()`에서 읽는 정도의 차이는 같은 헬퍼로 본다.
승격 시 **`cx.theme()`를 직접 읽는 형태**를 기본으로 하고(호출부마다 토큰을 넘기면 의미 매핑이
흩어진다), 구현이 미세하게 다르면(`px_2` vs `px_3`) 통일 대상을 정하되 **그 통일은 별도 커밋**으로
분리한다. 승격 후 **원본 정의는 반드시 삭제한다.**

### 승격 전 덮어쓰기 기록

`ButtonStyle`의 `border`/`hover`/`no_hover` 덮어쓰기는 **승격 전 화면을 그대로 두기 위한 것**이며,
G-001에서 제거했다. 아래 표는 변경 전 의도와 통일 판단을 보존하는 기록이다.

| 화면 | 덮어쓰기 | 원래 모습 | 통일안 |
| --- | --- | --- | --- |
| `file_sync` 버튼 8개 | `.hover(border)` / `.border(primary_hover)` / `.border(danger_active)` | 테두리 색 = hover 색 | 공용 생성자 기본값 사용 |
| `service_view` 버튼 4개 | `.border(t.border).no_hover()` | hover 반응 없음 | 공용 생성자 기본 hover 사용 |
| `service_mgr` 시작 버튼 | `.border(border).hover(secondary_hover)` (비활성 시) | 비활성도 테두리 유지 | `ButtonStyle::muted` 기본값 유지(테두리·hover 없음) |

G-001 판단: `muted`는 비활성 의미이므로 테두리와 hover를 추가하지 않고, 나머지는 각
`ButtonStyle` 생성자의 기본 상호작용을 사용한다.

---

## 측정 제외

| 대상 | 사유 |
| --- | --- |
| `app/assets/themes/*.json` | 데이터 파일 (테마 21종) |
| `Cargo.lock` | 자동 생성 |
| `target/` | 빌드 산출물 |

---

## 리팩터링 이력

| 날짜 | 대상 | 종류 | 이전 | 이후 | 비고 |
| --- | --- | --- | ---: | --- | --- |
| 2026-09-17 | 프로젝트 문서 | 정본 디렉터리 통합 | 루트 문서 5개 + 에이전트 규칙 중복 | `docs/` 5개 정본 + 루트 에이전트 어댑터 | 공통 내용은 `DEVELOPMENT_GUIDE.md`로 통합하고 어댑터에는 표면별 주의사항만 유지 |
| 2026-09-17 | `virtual_disk/mod.rs` | VDE-003 도메인 경계 추가 | VirtualBox 탐색 모델 없음 | 403줄의 플랫폼 비의존 모델·읽기 계약·오류 분류 | VDI 실제 파서와 GPUI 화면은 VDE-004 이후 연결 |
| 2026-09-21 | `virtual_disk/vdi.rs` | VDE-004 VDI 블록 리더 추가 | VDI 헤더·블록 맵 구현 없음 | 512줄의 read-only 헤더·맵 검증·동적/고정 블록 읽기 | 파티션·NTFS·잠금 안정성은 VDE-005~008에서 연결 |
| 2026-09-21 | `virtual_disk/vdi.rs` | VDE-005 안전 가드 추가 | 파일 잠금·원본 변경·실행 중 VM 대조 없음 | 782줄의 잠금 표식·환경변수 기반 VBoxManage 조회·크기/mtime 스냅샷·read-only 핸들 검증 | 800줄 주의 구간에 접근했으므로 VDE-006 전에 책임 단위 분할 후보를 검토 |
| 2026-09-21 | `virtual_disk/partition.rs` | VDE-006 MBR/GPT 파서 추가 | 파티션 검색 구현 없음 | 729줄의 MBR·EBR·GPT 양쪽 CRC·범위 검증과 `PartitionSource` 어댑터 | NTFS 파일시스템 판정은 VDE-007~008에서 연결 |
| 2026-09-21 | `virtual_disk/ntfs.rs` | VDE-007 NTFS 읽기 전용 어댑터 추가 | NTFS 디렉터리·스트림 접근 구현 없음 | 507줄의 파티션 범위 `Read + Seek`, NTFS 3.1 열거·기본 스트림 읽기·속성 보존과 env 주입 이미지 테스트 | 압축·암호화·항목별 오류 억제는 VDE-008에서 연결 |
| 2026-09-21 | `virtual_disk/ntfs.rs`·`virtual_disk/mod.rs` | VDE-008 NTFS 오류 경계 추가 | 파일 경로 없는 공통 오류와 압축·암호화 스트림 무방비 읽기 | `GuestEntry` 경로 래퍼, 파티션 범위 재검증, 압축·암호화 플래그 차단, 손상 메타데이터·원본 변경 보존, 오류 단위 테스트와 손상 이미지 테스트 | 항목별 복사 실패 알림은 VDE-012, 고정 손상 픽스처는 VDE-018에서 연결 |
| 2026-09-21 | `virtual_disk/path_policy.rs`·`virtual_disk/mod.rs` | VDE-009 호스트 경로 안전 매핑 추가 | 게스트 경로를 호스트 복사 대상에 연결하는 안전 경계 없음 | 절대 대상 루트, Windows 예약 이름·금지 문자·UTF-16 길이 제한, 게스트 리파스 포인트와 기존 호스트 링크 추적 차단, 6개 정책 테스트 | 실제 파일 복사·충돌·속성·오류 알림 연결은 VDE-010~012에서 수행 |
| 2026-09-21 | `virtual_disk/path_policy.rs`·`virtual_disk/copy.rs` | VDE-010 복사 엔진과 경로 정책 분리 | 경로 정책과 복사 엔진이 한 파일에 모여 952줄 경고 구간에 접근 | 경로 정책을 334줄 모듈로 분리하고 복사 엔진을 629줄로 유지, 청크 복사·부모 생성·건너뜀·덮어쓰기·새 이름 충돌과 13개 테스트 추가 | 파일 속성·항목별 실패 알림은 VDE-011~012에서 연결 |
| 2026-09-21 | `virtual_disk/metadata.rs`·`virtual_disk/ntfs.rs`·`virtual_disk/copy.rs` | VDE-011 메타데이터 적용 추가 | 게스트 속성과 시간 정보가 호스트 복사 결과에 반영되지 않음 | NTFS 생성·수정·접근 시간 모델링, Windows 속성·생성 시간·접근/수정 시간 적용, 비지원·권한 실패 수집, 5개 테스트 | 실제 VDI 이미지의 모든 메타데이터 조합과 UI 알림 연결은 VDE-019·VDE-012에서 후속 검증 |
| 2026-09-21 | `virtual_disk/issues.rs`·`virtual_disk/copy.rs` | VDE-012 오류 수집 도메인 경계 추가 | 복사 실패가 전체 작업 중단으로만 전달되고 반복 알림 억제 계약이 없음 | 오류 종류 분류, 형제 항목 계속 처리, `CopyReport.issues`, 경로·종류 기반 안정 억제 키와 중복 알림 차단, 11개 관련 테스트 | GPUI 토스트/로그 연결과 억제 상태 영속화는 VDE-013~019 후속 작업 |
| 2026-09-21 | `app/state.rs`·`app/virtual_disk_ops.rs`·`window/virtual_disk.rs`·`app/tests/virtual_disk.rs` | VDE-013 오프라인 VDI 탐색기 셸 추가 | VirtualBox 도메인이 GPUI 네비게이션과 연결되지 않음 | `ActivePanel`·`NAV_TOOLS` 등록, VDI 경로 입력, read-only VDI/파티션 검색, NTFS 선택 연결, 게스트 루트 경로·새로고침 카드, GPUI 회귀 테스트와 920/1280px 실제 캡처 | 실제 VDI 이미지 성공 경로와 파일 목록 E2E는 VDE-019에서 수행 |
| 2026-09-21 | `app/virtual_disk_ops.rs`·`window/virtual_disk.rs`·`app/tests/virtual_disk.rs` | VDE-014 탐색기 목록 조작 추가 | VDI 셸에 파일 행 조작과 경로 이동이 없음 | 숨김·시스템 항목을 필터링하지 않는 목록 행, 파일 종류·속성·크기 표시, 폴더 더블클릭, 상위 이동, Ctrl/Shift 선택 집합, 선택 카운트와 GPUI 상위 액션 회귀 테스트, 920/1280px 실제 캡처 | 실제 VDI 파일 행·숨김 항목·범위 선택 상호작용은 VDE-019와 VDE-016에서 검증 |
| 2026-09-21 | `app/virtual_disk_copy.rs`·`window/virtual_disk.rs`·`app/events.rs` | VDE-015 연속형 복사 작업 연결 | 탐색 목록에 대상 폴더·복사 진행 상태가 없음 | 대상 폴더 입력/네이티브 선택, 작업 스레드의 read-only VDI 재연결, 진행 이벤트·원자 중지 요청·완료/실패 요약, 단일 `scroll_pane` 복사 카드, 1000/920/1280px 실제 캡처 | 실제 이미지 복사 결과와 대용량 디렉터리 중지 세분화는 VDE-019에서 검증 |
| 2026-09-21 | `app/virtual_disk_ops.rs`·`window/virtual_disk.rs`·`app/tests/virtual_disk.rs` | VDE-017 안전·지원 상태 안내 | 실행 중 VM·잠금·미지원 파일시스템이 일반 읽기 실패와 구분되지 않음 | read-only 안전 경계 카드, 실행 중 VM/잠금 전용 오류, NTFS 3.1 지원 범위 안내, 미지원 파티션 경고, GPUI 상태 렌더 테스트와 920/1000/1280px 실제 캡처 | 실제 VBoxManage 실행 중 VM·미지원 이미지 E2E는 VDE-019에서 검증 |
| 2026-09-21 | `virtual_disk/partition.rs`·`virtual_disk/vdi.rs`·`virtual_disk/ntfs.rs`·`app/testdata/ntfs-testfs1.img` | VDE-018 고정 NTFS/합성 VDI 검증 | 실제 이미지에 연결된 파티션의 파일시스템 판정과 원본 불변성 통합 증거 없음 | MBR 파티션의 NTFS 3.1 부트 섹터 판정, 고정 NTFS read-only 열거, 손상 복사본 거부, 합성 동적 VDI에서 파티션·루트·파일 읽기 및 VDI 바이트 불변성 테스트 | GPUI 파일 행·키보드·복사 UI E2E는 VDE-019에서 검증 |
| 2026-09-21 | `app/virtual_disk_ops.rs`·`app/tests/virtual_disk.rs` | VDE-019 GPUI 목록·복사 상태 수용 테스트 착수 | 실제 파일 목록을 주입할 UI 테스트 seam과 복사 상태 렌더 검증이 없음 | `GuestFileSource` trait object 테스트 경계, 숨김·시스템 항목 포함 목록 렌더와 Ctrl+A 선택, 복사 진행·중지·실패 요약 카드 GPUI 테스트 2개 추가; 전체 137 passed·4 ignored | 실제 VirtualBox 패널 캡처·foreground 입력·복사 대상 E2E는 VDE-019 잔여 |
| 2026-09-22 | `app/src/app/mod.rs`·`app/src/app/virtual_disk_ops.rs`·`app/src/app/virtual_disk_copy.rs`·`app/src/virtual_disk/vdi.rs`·`app/src/window/virtual_disk.rs`·`scripts/Invoke-ClaudeVisualCheck.ps1` | VDE-019 격리 합성 VDI 릴리스 E2E 보강 | 기존 하네스가 VirtualDisk 초기 패널과 실제 VDI 시드·복사 대상 검증을 지원하지 않음 | `-SeedVdi`·`-AutoCopyVdi`·`-InitialPanel VirtualDisk`와 검증 전용 환경 변수를 추가하고, 합성 NTFS VDI를 실제 release 앱에서 열어 숨김/시스템 항목 17개·937,234바이트 복사와 손상 항목 사유를 확인; 캡처 2건·전체 152 passed·4 ignored·Clippy 기존 경고 7건·53개 파일·20,690줄 기준 지도 갱신 | 실제 VBox 생성 VDI와 독립 Visual Reviewer는 VDE-019 잔여; 구현 commit `87937dc` push 완료 |
| 2026-09-21 | `docs/VIRTUAL_DISK_BACKENDS.md` | VDE-021 실행 중 VM 백엔드 설계 검토 | `guestcontrol`을 오프라인 VDI 경로와 혼용할 위험과 실제 명령·자격 증명 경계 미정 | Oracle 공식 명령 계약, `GuestFileSource`/전송 capability 분리, read-only 명령 목록, credential 비저장, 취소·시간 제한·출력 제한, 격리 VM 선행 조건 기록 | `VBoxManage.exe` 미설치로 실제 Guest Control E2E는 후속; VDE-019 완료 전 구현 금지 |
| 2026-09-22 | `app/src/sync/tests.rs`·`app/src/config.rs`·`docs/PROJECT_MAP.md` | T-001·T-002 회귀 테스트와 구조 지도 실측 갱신 | 읽기 전용 대상 덮어쓰기와 `update_config` 필드 보존을 전체 테스트에서 보장하지 않음; 일부 모듈 줄 수가 이전 기록과 불일치 | 두 회귀 테스트 추가, 격리 설정 경로 검증, 전체 139 passed·4 ignored·Clippy 기존 경고 7건, 50개 파일·19,486줄 기준으로 지도 갱신 | 전체 `cargo fmt --check`는 기존 baseline 불일치로 별도 보류; 기능 변경 없이 테스트·문서만 반영 |
| 2026-09-22 | `app/src/window/ui.rs`·`file_sync.rs`·`interval.rs`·`service_mgr.rs`·`service_view.rs`·`app/mod.rs`·`scripts/Invoke-ClaudeVisualCheck.ps1` | G-001 버튼 스타일 덮어쓰기 정리 | 패널별 `border`·`hover`·`no_hover` 덮어쓰기로 공용 버튼 의미가 화면마다 달랐고 확장 메서드가 dead-code가 됨; foreground 안전 차단으로 패널 전환 캡처가 불가능했음 | 세 패널을 생성자 기본값으로 통일하고 확장 메서드 제거, 검증 전용 `-InitialPanel FileSync`·`-InitialPanel AutoStart` 경로 추가, 관련 GPUI 테스트·전체 145 passed·4 ignored·Clippy 기존 경고 7건, 52개 파일·20,101줄 기준 지도 갱신 | 격리 release 파일 동기화 캡처 `g001-file-sync-013019.png`·자동 시작 캡처 `g001-auto-start-013004.png` 성공, processCount=0·sessionCount=0; 독립 Visual Reviewer는 후속 |
| 2026-09-22 | `Cargo.toml`·`app/Cargo.toml`·`Cargo.lock` | D-009 notify 직접 의존성 승격 | 실시간 감시 예정 기능이 `gpui-component`의 전이 의존성에만 기대고 있어 앱의 의존성 계약이 불명확함 | workspace/app에 `notify 7` 직접 의존성을 선언하고 lockfile을 갱신, `cargo check --locked`·전체 145 passed·4 ignored·Clippy 기존 경고 7건·직접 `cargo tree` 확인; 실시간 감시 동작은 D-010~D-013에서 구현 | 앱 동작 변경 없이 Cargo 경계만 고정; commit `0315bbd` push 완료 |
| 2026-09-22 | `app/src/config.rs` | D-010 동기화 감시 방식 스키마 | 작업별 감시 방식이 설정 모델에 없어 주기 모드와 실시간 모드를 구분할 수 없음 | `WatchMode::{Interval, Realtime}`·`SyncJob.watch_mode`·구버전 기본값·snake_case 왕복 테스트 추가, 전체 146 passed·4 ignored·Clippy 기존 경고 7건, 52개 파일·20,141줄 기준 지도 갱신 | 실제 이벤트·디바운스·설정 UI는 D-011~D-013에서 구현; commit `15e9765` push 완료 |
| 2026-09-22 | `app/src/app/watch.rs`·`app/src/app/background.rs`·`app/src/app/tests/file_sync.rs` | D-011 실시간 파일 감시·디바운스 | 실시간 감시 스키마만 있고 변경 이벤트를 동기화 실행으로 연결하지 않음 | `notify` 재귀 watcher, 작업별 2초 quiet debounce, 경로·작업 변경 시 watcher 정리, 실제 임시 폴더 파일 변경 통합 테스트와 디바운스 단위 테스트, 전체 151 passed·4 ignored·Clippy 기존 경고 7건, 53개 파일·20,606줄 기준 지도 갱신 | 구현 커밋 `e68a3f6` push 완료 |
| 2026-09-22 | `app/src/app/watch.rs`·`app/src/app/state.rs`·`app/src/app/events.rs`·`app/src/app/tests/file_sync.rs` | D-012 감시 실패 자동 강등 | watcher 생성·경로 등록 실패가 반복되거나 작업이 실시간 모드에 남을 수 있음 | 작업별 실패 중복 억제, `SyncWatchFallback` 이벤트, `Interval` 강등·설정 저장·WARN 로그·경고 토스트, 단위·GPUI 회귀 테스트, 전체 151 passed·4 ignored·Clippy 기존 경고 7건 | 구현 커밋 `e68a3f6` push 완료 |
| 2026-09-22 | `app/src/window/file_sync.rs`·`app/src/app/tests/file_sync.rs`·`scripts/Verify-Workspace.ps1` | D-013 감시 방식 선택 UI | 사용자가 작업별 주기/실시간 방식을 선택할 수 없음 | `실시간 감시` 토글, 실시간 모드의 주기 선택 숨김, 작업 카드 상태 표시, 필수 GPUI 20개와 릴리스 `CLAUDE_LOCAL` 캡처 `d011-realtime-panel-015924.png`, processCount=0·sessionCount=0 | 구현 커밋 `e68a3f6` push 완료 |
| 2026-09-22 | `app/src/app/ui_state.rs`·`app/mod.rs`·기능별 참조 호출부 | G-002 `AppRoot` 기능 상태 묶음 | 서비스·동기화·광고 스크롤과 작업 상태가 루트 엔티티 필드에 혼재해 소유 경계가 흐림 | GPUI 엔티티는 하나로 유지하고 `ServiceState`·`SyncState`·`AdBlockState` 일반 구조체로 상태 소유권 분리, 전체 139 passed·4 ignored·Clippy 기존 경고 7건, 51개 파일·19,496줄 기준 지도 갱신 | 동작 변경 없는 구조 리팩터링; 실제 화면 검증은 UI 동작 변경이 없어 N/A |
| 2026-09-22 | `app/src/config.rs`·`app/src/app/mod.rs`·`app/tests/mod.rs` | G-003 스플리터 폭 영속화 | 앱 재시작 시 사용자가 조정한 사이드바 폭이 기본 240px로 돌아감 | `sidebar_width` 설정 필드·200~360px 보정, `ResizableState::sizes()` mouse-up 저장, 시작 시 복원, 설정/GPUI 회귀 테스트·전체 140 passed·4 ignored·Clippy 기존 경고 7건, 51개 파일·19,558줄 기준 지도 갱신 | 격리 release 기본 캡처 `g003-default-003709.png`·320px 시드 복원 캡처 `g003-restored-width-003742.png` 성공; 하네스가 divider 드래그를 지원하지 않아 실제 저장 콜백과 독립 Visual Reviewer는 후속 |
| 2026-09-22 | `app/src/config.rs`·`app/src/app/mod.rs`·`app/src/app/virtual_disk_ops.rs`·`app/src/app/virtual_disk_copy.rs`·`app/src/window/virtual_disk.rs`·`app/src/app/tests/virtual_disk.rs` | VDE-012 GPUI 오류 알림 연결 | 도메인 오류 보고는 있었지만 화면 상세 로그·반복 알림 억제 상태·항목별 UI 조작이 연결되지 않음 | 항목별 상세 로그, 미억제 오류에만 반복 토스트, 요약 카드의 억제/재표시 버튼, `AppConfig` 저장·복원, 관련 GPUI 직접 상태 전환 테스트와 전체 140 passed·4 ignored·Clippy 기존 경고 7건, 51개 파일·19,707줄 기준 지도 갱신 | 격리 release 기본 캡처 `vde012-default-004932.png` 성공; 실제 VDI 오류 카드·버튼 click dispatch·복사 대상 E2E는 실제 이미지/하네스 제약으로 미확인하며 VDE-019에서 후속 |
| 2026-09-22 | `app/src/sync/mod.rs`·`app/src/sync/tests.rs` | D-017 심볼릭 링크 건너뜀 계상 | 심볼릭 링크·정션을 복사하지 않으면서 실패로 집계해 정상 동기화와 오류가 섞임 | 링크를 따라가지 않고 `skipped`로 계상하며 진행 콜백에 반영, 실제 Windows 심볼릭 링크 회귀 테스트와 전체 141 passed·4 ignored·Clippy 기존 경고 7건, 51개 파일·19,731줄 기준 지도 갱신 | 현재 정책은 안전 `Skip` 고정이며 `Follow`·`Recreate` 옵션과 권한 범위는 사용자·제품 판단으로 D-014~016에 남김; commit `6cf305c` push 완료 |
| 2026-09-22 | `app/src/config.rs`·`app/src/sync/mod.rs`·`app/src/sync/tests.rs` | D-015 링크 처리 모드 스키마 경계 | 링크 정책을 확장할 때 설정·엔진·구버전 복원 계약이 없으면 미구현 모드가 조용히 무시될 수 있음 | `SymlinkMode::{Skip, Follow, Recreate}`·`SyncJob.symlink_mode`와 snake_case/구버전 Skip 테스트 추가; Follow/Recreate는 명시적 실패로 남김; 전용 테스트·cargo check·git diff --check 통과, 53개 파일·20,789줄 기준 지도 갱신 | 실제 Follow/Recreate·UI 옵션은 원본 루트 탈출·순환 링크·권한 정책 판단 후 D-014·D-016에서 구현; commit `38dddb3` push 완료 |
| 2026-09-22 | `app/src/virtual_disk/vdi.rs`·`app/src/virtual_disk/partition.rs`·`app/src/virtual_disk/ntfs.rs`·`app/src/app/mod.rs`·`scripts/Invoke-ClaudeVisualCheck.ps1` | VDE-019 실제 VirtualBox VDI 릴리스 E2E와 표준 헤더 수정 | 앱 파서가 VirtualBox 표준 헤더의 블록 필드를 4~8바이트 앞에서 읽어 실제 VDI를 거부함 | 표준 VDI 1.1 오프셋으로 파서·fixture writer를 수정하고 raw NTFS fixture export를 추가; VBoxManage 7.2.14 `convertfromraw` VDI를 release 앱에서 열어 17개·937,234바이트 복사와 손상 항목 요약을 확인; 전체 155 passed·4 ignored, VDI 60 passed·2 ignored, 캡처·정리 증거 기록 | 외부 키보드 입력·독립 Visual Reviewer·Guest Control VM은 후속; implementation commit `bed5914` push 완료 |
| 2026-09-22 | `app/testdata` export fixture·`scripts/Invoke-ClaudeVisualCheck.ps1`·`docs/VERIFICATION.md` | VDE-019 릴리스 재검증과 합성 fixture 갱신 | 기존 검증용 합성 VDI가 이전 헤더 계약으로 남아 최신 릴리스 파서에서 거부됨 | `exports_ntfs_vdi_fixture_when_requested`로 fixture를 격리 경로에 재생성하고 표준/합성 VDI 모두 파일 목록·17개 복사·손상 항목 사유를 최신 release에서 확인; 캡처 `continuation-vde019-standard-release-031918.png`, `continuation-vde019-synthetic-refreshed-032017.png`; 세션 정리 후 process/session 0 | 오류 억제 버튼 클릭·외부 키보드·독립 Visual Reviewer는 포그라운드 안전 차단 또는 별도 검토가 필요 |
| 2026-09-22 | `scripts/Invoke-ClaudeVisualCheck.ps1`·`docs/TODO.md`·`docs/VERIFICATION.md`·`docs/MASTER_PLAN.md` | VDE-016·019 외부 키보드 검증 경계 정리 | 실제 VDI 행을 대상으로 한 하네스 키 입력 경로와 선택 상태 증거가 분리되어 있지 않음 | `Key Ctrl+A`와 `-SelectAllVdi`를 하네스에 추가하고 실제 표준 VDI에서 선택 전 `0개`, 시드 선택 후 `16개` 캡처를 확인; 포그라운드가 대상 창이 아니어서 안전 차단된 `Key Ctrl+A` 시도와 세션 `PROCESS_COUNT=0`·`SESSION_COUNT=0`·임시 루트 삭제를 기록 | 외부 키보드 E2E·독립 Visual Reviewer는 미검증; PowerShell 구문 분석 통과 |
| 2026-09-22 | `app/src/app/tests/virtual_disk.rs`·`docs/TODO.md`·`docs/VERIFICATION.md` | VDE-012 오류 억제 버튼 dispatch 검증 보강 | 기존 테스트가 억제 상태 메서드를 직접 호출해 실제 GPUI 버튼 이벤트 경로를 확인하지 못함 | 요약 카드 버튼을 `simulate_click`로 눌러 억제와 재표시를 모두 확인하고, 버튼이 화면 밖으로 밀리지 않도록 1200×1000 테스트 창으로 검증; 전체 155 passed·4 ignored, `cargo check --locked`, Clippy exit 0(기존 경고 7건) | 실제 릴리스 앱 버튼 조작과 독립 Visual Reviewer는 VDE-012 후속 |
| 2026-09-22 | `app/src/main.rs`·`app/src/window/service_view.rs`·`app/src/platform/windows/tray.rs`·`app/src/virtual_disk/vdi.rs` | Clippy baseline 정리 | 기존 표준 Clippy 경고 7건이 품질 게이트에 남아 있음 | 모듈 문서 주석·불필요한 mutable FFI 참조·수동 나머지 검사·중복 클로저를 정리하고 표준 Clippy 경고 0건, 전체 155 passed·4 ignored, `cargo check --locked` 확인 | 전체 rustfmt check의 기존 baseline 포맷 차이는 별도 정리 범위 |
| 2026-09-22 | `scripts/Verify-Workspace.ps1`·`docs/VERIFICATION.md`·`docs/MASTER_PLAN.md` | IDE 안전 통합 검증 재실행 | 코드·필수 GPUI 테스트·전체 테스트 결과를 한 번에 확인한 최신 증거가 문서에 연결되지 않음 | 필수 GPUI 테스트 25개와 전체 테스트 155 passed·4 ignored를 재실행하고 `IDE_VERIFIED` 출력 확인 | 데스크톱 표면은 `DESKTOP_PENDING`; 포그라운드 조건 확보 후 별도 하네스 실행 |
| 2026-09-22 | `scripts/Assert-ProjectDocs.ps1`·`scripts/Verify-Workspace.ps1` | 활성 TODO·검증 매트릭스 자동 정합성 게이트 | 새 작업 ID가 TODO에만 추가되거나 검증 매트릭스에 중복으로 기록될 수 있음 | 중첩 항목을 포함한 활성 ID 27개와 매트릭스 ID 27개의 중복·누락·고아 항목을 자동 검사하고 `Verify-Workspace.ps1` 첫 단계에 연결 | `DOCS_VERIFIED active=27 matrix=27`; 향후 작업 ID 변경은 코드 테스트 전에 즉시 실패 |
| 2026-09-22 | `scripts/Invoke-ClaudeVisualCheck.ps1`·`docs/TODO.md`·`docs/VERIFICATION.md` | VDE-016·019 포그라운드 입력 한계 재검증 | 기본 `SetForegroundWindow` 경로만으로는 실제 VDI 목록에 외부 키를 보낼 수 없음 | UI Automation `SetFocus`와 `SetWindowPos` 최상위 전환을 추가 시도했지만 현재 데스크톱 포그라운드가 유지됨; 하네스 안전 차단으로 입력 누출을 방지하고 세션 종료 후 프로세스·세션 0과 임시 루트 삭제 확인 | 외부 키보드 E2E와 독립 Visual Reviewer는 미검증; 별도 사용자 포커스/검증 표면이 필요 |
| 2026-09-22 | `docs/TODO.md`·`docs/VERIFICATION.md`·`docs/MASTER_PLAN.md` | VDE-013 완료 승격 | 실제 VDI 성공 경로 증거가 VDE-019에 추가되었지만 VDE-013 완료 기록과 활성 대기열이 남아 있었음 | VDI 경로·MBR/NTFS 파티션·게스트 루트 연결 캡처와 GPUI 새로고침 테스트를 근거로 VDE-013을 완료 이력으로 이동; 활성 ID·검증 매트릭스 `26/26`, 전체 테스트 기준 157 passed·4 ignored | 폴더 진입·범위 선택·외부 키보드 등은 VDE-014~016의 잔여 범위 |
| 2026-09-22 | `app/src/virtual_disk/vdi.rs`·`scripts/Invoke-ClaudeVisualCheck.ps1`·`docs/TODO.md`·`docs/VERIFICATION.md` | VDE-017 실제 미지원 파티션 릴리스 E2E | GPUI 상태 렌더와 단위 fixture만 있어 실제 release 앱에서 미지원 파티션 메시지를 확인하지 못함 | `exports_unsupported_partition_vdi_fixture_when_requested`로 MBR 타입 `0x83`·NTFS 시그니처 없는 VDI를 격리 생성하고, `reads_unsupported_partition_fixture_without_claiming_ntfs`로 NTFS 오인 방지를 확인; release 캡처 `vde017-unsupported-partition-release-fixed-033450.png`에서 오류 카드·파티션 경고 확인, 전체 157 passed·4 ignored·Clippy exit 0, process/session 0 | 실제 실행 중 VM E2E와 독립 Visual Reviewer는 후속 |
| 2026-09-22 | `app/src/sync_history.rs`·`app/src/app/background.rs`·`app/src/main.rs` | D-018 동기화 실행 이력 저장 | 실행 결과가 화면 로그와 설정 상태에만 남아 앱 재시작 후 이력을 조회할 파일이 없음 | `sync-history.json`에 실행 순서·작업 식별자·시각·결과 건수·중지·요약을 append하고 손상 JSON은 덮어쓰지 않음, 전용 2개 테스트와 전체 143 passed·4 ignored·Clippy 기존 경고 7건, 52개 파일·19,884줄 기준 지도 갱신 | 이력 보존 정책은 로그와 동일하게 D-019에서, UI 목록은 D-020에서 후속; commit `97009c3` push 완료 |
| 2026-09-22 | `app/src/sync_history.rs` | D-019 동기화 이력 보존 정책 | JSON 이력은 추가만 되어 오래된 실행 결과가 무한히 남을 수 있음 | `LogConfig.max_age_days`·`max_files`로 기간·개수 초과분을 새 append 전에 제거하고 최소 한 건을 유지, 보존 경계 테스트·전체 144 passed·4 ignored·Clippy 기존 경고 7건, 52개 파일·19,946줄 기준 지도 갱신 | 파일 용량 롤링은 JSON 이력에 적용하지 않으며 최근 이력 화면은 D-020 후속; commit `ed71af4` push 완료 |
| 2026-09-22 | `app/src/sync_history.rs`·`app/src/app/mod.rs`·`app/src/app/events.rs`·`app/src/window/file_sync.rs`·`app/src/app/tests/file_sync.rs`·`scripts/Invoke-ClaudeVisualCheck.ps1` | D-020 최근 동기화 이력 카드 부분 구현 | 저장된 실행 결과를 앱 재시작 후 패널에서 볼 수 없고 완료 직후 목록 갱신도 없음 | 최신 20건 로드·완료 이벤트 새로고침·상태/결과 건수/소요 시간 카드·폭 회귀 테스트와 GPUI 시드 렌더 테스트, `-SeedHistory`·`-InitialPanel FileSync` 검증 경로 추가, 전체 145 passed·4 ignored·Clippy 기존 경고 7건, 52개 파일·20,101줄 기준 지도 갱신 | 격리 release 시드 패널 캡처 `d020-history-panel-012458.png`에서 중지/실패/성공 3행 확인, processCount=0·sessionCount=0; 독립 Visual Reviewer는 후속; commits `031148e`, `ff9dc72` push 완료 |
| 2026-07-29 | 편의 기능 스플리터 3곳 | 공용 레이아웃 승격 | 패널별 고정 초기 폭 | `window::balanced_split` | 설정 pane 과도 축소 방지, 양쪽 가용폭 사용 |
| 2026-07-29 | `app.rs` | 책임 단위 분할 + 재배치 | 1,798 | `app/` 7파일 (최대 564) | 대시보드·로그 렌더는 소유가 잘못돼 있어 `window/`로 이동 |
| 2026-07-29 | `platform/windows.rs` | 책임 단위 분할 + 승격 | 1,361 | `platform/windows/` 6파일 (최대 344) | `wide_null`을 `windows/mod.rs`로 **공용 승격** |
| 2026-07-29 | 배지·액션 버튼 | **중복 제거 (공용 승격)** | 4파일 합 2,121 | 4파일 합 1,981 + `ui.rs` 194 | 정의 4개 + 인라인 8곳 → `ui.rs` 하나로 |
| 2026-07-29 | 테마·스위치·스크롤 | 공용 정책 승격 + 레이아웃 교정 | 35/36 테마에 스위치 토큰 없음 | 런타임 대비 보정 + 공용 스위치 + 36종 감사 | 파일 동기화 컨텐츠의 고정 높이를 제거해 자동 스크롤 복구 |
| 2026-07-30 | 앱 셸·파일 동기화 | 조작 흐름 재설계 | 고정 사이드바 + 파일 동기화 이중 pane | 리사이즈 사이드바 + 전체 너비 단일 스크롤 | 실행 시 현재 입력 자동 저장·완료 이벤트 UI wake 포함 |
| 2026-07-30 | 통계 타일·설정 행·프리셋 칩 | **중복 제거 (공용 승격)** | 4파일 합 1,933 | 4파일 합 1,721 + `ui.rs` 207→300 | 정의 2개 + 인라인 복제 다수 → `ui::stat_tile`·`option_row`·`choice_chip` |
| 2026-07-30 | `sync.rs` | 책임 단위 분할 | 831 🟡 | `sync/` 2파일 (435 / 399) | 본문과 단위 테스트 분리 |
| 2026-07-30 | `app/tests.rs` | 책임 단위 분할 | 929 🟡 | `app/tests/` 4파일 (최대 448) | 픽스처는 `mod.rs`가 단독 소유, 시나리오별 하위 모듈 |
| 2026-07-30 | 주기 선택 UI | **기능 재설계 + 통일** | 패널별 고정 프리셋 칩 | `app/interval.rs` + `window/interval.rs` + `util.rs` | 드롭다운 + 사용자 정의 프리셋. 칩 패딩·주기 표기도 함께 통일(화면 변경 의도됨) |
| 2026-07-30 | 사이드바 전역 스위치 | 공용 승격 | 광고 차단 전용 인라인 | `AppRoot::render_sidebar_switch` | 파일 동기화 스위치를 같은 모양으로 추가하며 승격 |
| 2026-07-30 | 로그 레벨 칸 | **중복 제거 (공용 승격)** | `dashboard` 64px / `log_view` 72px | `ui::log_level_label` (76px) | 좁은 쪽에서 `SUCCESS`가 접혀 잘렸다. 레벨→색 매핑 중복도 함께 흡수 |
| 2026-07-30 | `scroll_pane` | 레이아웃 고정 | 세로만 스크롤, 가로 `visible` | `overflow_x_hidden` 추가 | 컨텐츠가 뷰포트 폭 대신 내용 폭으로 잡히는 경로 차단(가설 기반) |

앞의 두 작업은 **동작 변경 없이** 수행했다. 세 번째 작업은 아래 한 건을 제외하면 화면이 동일하다.

- `service_view`의 등록/지금 실행 버튼이 `sidebar_primary` → `primary` 토큰으로 바뀌었다.
  사이드바 토큰을 패널 버튼에 쓰던 것이 의미 매핑 규칙 위반이라 승격하면서 교정했다.

세 작업 모두 `cargo check`(경고 0) · `cargo test`(12 passed / 1 ignored)로 확인했다.

**줄 수 총합은 55줄 늘었다**(`ui.rs`의 문서 주석 때문). 대신 패널 4개가 140줄 줄었다.
공용 승격의 목적은 총량 감소가 아니라 **변경 지점을 하나로 만드는 것**이다.

### 2026-07-30 리팩터링 (🟡 경고 해소)

정본의 순서대로 **① 중복 제거 → ③ 책임 단위 분할**을 수행했다. ②(오배치 이동)는 대상이
없었다. ①만으로는 `sync.rs`·`app/tests.rs`가 임계값 아래로 내려가지 않아 ③까지 진행했다.

- 패널 4개 212줄 감소(`ad_block` −57, `settings` −54, `file_sync` −42, `dashboard` −14),
  `ui.rs`는 93줄 증가. 결과적으로 🟡 경고 **2건 → 0건**, 최대 파일 929 → 656줄
- **동작 변경 없음**: `cargo check` 경고 0, 34개 테스트 통과, 실제 앱 캡처로
  광고 차단·설정 패널이 승격 전과 동일하게 그려지는 것을 확인
- 분할로 테스트 경로가 `app::tests::<name>` → `app::tests::<모듈>::<name>`으로 바뀌어
  `Verify-Workspace.ps1`의 필수 테스트 매칭을 모듈 경로에 의존하지 않도록 함께 고쳤다

---

## 다음 리팩터링 후보

우선순위는 줄 수가 아니라 **구조 개선 효과** 순이다.

### 1순위 — 남은 중복 제거 (줄 수 무관, 지금 처리 가능)

**추적 중인 승격 후보 없음.** 새로 발견하면 「중복 헬퍼 추적」 표에 먼저 적는다.

다만 **`debug_selector` 래퍼**는 관찰 대상이다. `ui::action_button`(`Stateful<Div>`)과
`ui::toggle_switch`(`Switch`)가 debug_selector를 달지 않아, 테스트에서 클릭해야 하는 곳마다
`div().debug_selector(..)`로 감싸는 한 줄이 14곳에 복제돼 있다. 헬퍼 안으로 옮기려면 `id`를
`impl Into<ElementId>`에서 문자열로 좁혀야 하는데, `("svc-start", ix)`처럼 튜플 id를 쓰는
호출부 3곳이 걸린다. **한 줄짜리 관용구라 지금은 승격하지 않는다.** id 체계를 손볼 일이
생기면 함께 처리한다.

### 2순위 — 책임 재배치·분할 (중복 제거 후에도 크면)

- **`app/mod.rs` (790)** — 800줄 경고까지 10줄 남았다. `AppRoot` 필드가 30개에
  가깝다. `TODO.md`의 「`AppRoot` 분할 검토」와 같은 항목이다. 다음에 커지면 사이드바 렌더를
  `app/sidebar.rs`로 떼는 것이 가장 자연스럽다.
- **`app/tests/file_sync.rs` (794)** — 시나리오가 늘어 800줄 경고에 근접했다. 다음 감시·이력 시나리오가 추가되면
  레이아웃/조작/영속화로 다시 나눈다.
- **`window/service_mgr.rs` (670)** — 가상 리스트 행 렌더가 큰 비중을 차지한다.
  800줄에 닿으면 행 렌더를 `service_mgr/row.rs`로, 보기 설정을 `service_mgr/settings.rs`로 분리한다.
- **`window/file_sync.rs` (727)** — 진행률·최근 이력·감시 방식 설정 UI가 들어가면 커질 수 있다.
  작업 선택·설정·실패 기록의 렌더 책임을 의미별 하위 모듈로 나누는 것이 자연스럽다.
