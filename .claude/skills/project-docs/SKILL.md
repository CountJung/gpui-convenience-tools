---
name: project-docs
description: Update the canonical project documents under docs/ without duplicating them in agent adapters.
argument-hint: [init|update]
disable-model-invocation: true
allowed-tools: Read, Write, Edit, Glob, Grep, Bash
---

# Project Docs

이 저장소의 프로젝트 문서는 반드시 `docs/` 아래에 둔다. 루트에는 `AGENTS.md`와 `CLAUDE.md`
에이전트 어댑터만 남긴다.

## 정본 문서

| 경로 | 역할 |
| --- | --- |
| `docs/README.md` | 사용자 안내와 문서 인덱스 |
| `docs/DEVELOPMENT_GUIDE.md` | 공통 개발·아키텍처·검증 규칙 |
| `docs/MASTER_PLAN.md` | 단계 계획과 완료 이력 |
| `docs/PROJECT_MAP.md` | 실제 구조·줄 수·공용 유틸 지도 |
| `docs/TODO.md` | 고유 ID 기반 미완료 작업 대기열 |

## 갱신 절차

1. `docs/README.md`에서 현재 문서 역할을 확인한다.
2. 코드·스크립트·CI의 현재 사실을 확인한 뒤 해당 정본만 수정한다.
3. 다른 문서에는 내용을 복사하지 않고 canonical 문서의 경로를 참조한다.
4. 구조가 바뀌면 `docs/PROJECT_MAP.md`의 파일 목록·줄 수·책임을 갱신한다.
5. 단계가 완료되면 작업 항목을 `docs/TODO.md`에서 제거하고 `docs/MASTER_PLAN.md`에 기록한다.
6. 문서 이동 후 루트에 `AGENTS.md`, `CLAUDE.md` 외의 Markdown/HTML/TXT 문서가 남지 않았는지,
   오래된 경로 참조가 없는지 확인한다.

## 에이전트 어댑터 규칙

`AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`와 각 에이전트/스킬 파일에는
공통 규칙을 복사하지 않는다. 해당 에이전트가 자주 잊는 표면별 주의사항과 `docs/` 참조만
기록한다.
