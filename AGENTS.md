# AGENTS

**gpui-convenience-tools** — Rust + GPUI 기반 다용도 데스크탑 보조 도구 모음.
편의 기능을 독립 패널로 모아 두는 것이 이 저장소의 정체성이다.

코딩 지침의 **단일 정본**은 다음 파일이다.

- `.github/copilot-instructions.md`

이 문서에는 규칙을 중복 작성하지 않는다. 세부 규칙 변경은 메인 지침 문서에서만 수행한다.

## Reference Map

| 문서 | 역할 |
| --- | --- |
| `.github/copilot-instructions.md` | 코딩 지침 단일 정본 |
| `.github/instructions/gpui-core.instructions.md` | 파일별 자동 적용 브리지 |
| `.github/agents/error-reviewer.agent.md` | 오류 분석 서브 에이전트 |
| `.github/agents/ui-visual-reviewer.agent.md` | Codex/Copilot 공용 독립 시각 검증 프롬프트 |
| `.claude/agents/ui-visual-reviewer.md` | Claude Code 시각 검증 서브 에이전트 어댑터 |
| `.github/skills/gpui-rust-ui/skill.md` | GPUI UI 생성 스킬 (아래 주의 참조) |
| `MasterPlan.md` | 아키텍처 원칙 · 단계 계획 · **완료 이력** |
| `PROJECTMAP.md` | 구조 · 크기 · 공용 유틸 추적 (1,000줄 = 구조 리팩터링 트리거) |
| `TODO.md` | 미착수 구현 대기열 |
| `CLAUDE.md` | Claude Code용 저장소 안내 |
| `README.md` | 사용자 문서 |

## 주의: 문서 간 우선순위

`.github/skills/gpui-rust-ui/skill.md`는 GPUI 작업 절차를 보조하지만 **메인 지침이 우선한다**.
카드 표면은 `secondary`/`list`, 위험 상태는 `danger`를 사용한다. 스킬 문서의 Windows 빌드
트러블슈팅 절(매니페스트 중복, 콘솔 서브시스템)은 여전히 유효하다.
