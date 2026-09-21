# project-docs 설치

## 개인용 (모든 프로젝트에서 사용 — 추천)
이 폴더 전체를 다음 위치에 복사:

    ~/.claude/skills/project-docs/

## 프로젝트 전용 (팀과 git으로 공유)
이 폴더 전체를 다음 위치에 복사:

    <repo>/.claude/skills/project-docs/

## 사용
Claude Code(터미널 또는 VSCode 확장) 세션에서:

    /project-docs                  # docs/ 아래의 정본 문서 생성/갱신 (init)
    /project-docs init ./my-app    # 특정 경로에 생성
    /project-docs update           # 기존 문서를 리포 최신 상태로 갱신

이 저장소에서는 루트에 프로젝트 문서를 만들지 않는다. `docs/`가 문서 정본 디렉터리이고,
루트에는 `AGENTS.md`와 `CLAUDE.md` 에이전트 어댑터만 유지한다. 공통 내용은 어댑터에 복사하지
않고 `docs/DEVELOPMENT_GUIDE.md`를 참조한다.

VSCode에서는 Claude Code 확장이 .claude/skills 및 ~/.claude/skills를 자동으로 스캔하므로,
채팅창에 / 를 입력하면 자동완성 목록에 project-docs가 그대로 나타납니다. 별도 설정은 필요 없습니다.

## 이름을 바꾸고 싶다면
폴더 이름과 SKILL.md의 `name:` 필드를 동일하게 함께 바꾸세요 (예: docset, repo-docs).
둘이 어긋나면 호출 이름이 예상과 달라질 수 있습니다.
