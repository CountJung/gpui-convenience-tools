# GitHub Copilot Main Instructions

## ����

??����??gpui-convenience-tools ????��??�� GitHub Copilot????��????�� ���� ��??����??
��� ����, ����, ���� ����?? ??����??��????�� ??��??��.

## ??�� ����

- Rust ??�� �ڵ� ??��
- GPUI UI ����
- gpui-component ??�� ??��
- Windows ??��????�� �ڵ�
- ??ũ??��??�� ??�� ??��??���� ���� ����

## ??�� ??�� ??Ģ

- ??����??�ڵ� ��ħ�� ??�� ??��??�� ??????��.
- ??�� ����??�� ��Ģ??�ߺ� ??��???? ??��, ??����??��ũ??��.
- ��Ģ ����?? ��?? ??����??�ݿ�???? ���� ����??��ũ????????��.

## ���� ��??

- MasterPlan ??�� ��????�� ??�� ����??����??��.
- ??�� ��ǥ??����??�� ??Ű??ó ����?? ??��????û????�� ????��???? ??��??
- TASKS ���� ??��????�� ??�� ��????�� ����??��.

## GPUI ????�� ��??

- GPUI 0.2.2, gpui-component 0.5.1 ??ȯ API????��??��.
- ??��?? �ݵ�??cx.theme().??ū ���??�� ??��??��.
- ??���ڵ� ??���� ??��???? ??��??
- ???? ��� ??ū ����????????��.
  # GitHub Copilot Main Instructions

  ## 목적

  이 문서는 gpui-convenience-tools 저장소에서 GitHub Copilot이 따라야 하는 메인 지침 문서다.
  모든 구현, 리뷰, 문서 갱신은 이 문서를 기준으로 수행한다.

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
  - TASKS 진행 상태는 실제 완료 기준으로 갱신한다.

  ## GPUI 및 테마 기준

  - GPUI 0.2.2, gpui-component 0.5.1 호환 API만 사용한다.
  - 색상은 반드시 cx.theme().토큰 방식으로 사용한다.
  - 하드코딩 색상값을 사용하지 않는다.
  - 의미 기반 토큰 매핑을 유지한다.
    - 페이지 배경: background
    - 기본 텍스트: foreground
    - 주요 액션: primary, primary_foreground
    - 보더: border
    - 사이드바: sidebar, sidebar_foreground, sidebar_primary, sidebar_primary_foreground, sidebar_accent
    - 카드 유사 표면: secondary 또는 list
    - 위험 상태: danger
  - card, destructive 토큰에 의존하지 않는다.

  ## UI 구성 기준

  - h_flex, v_flex 중심으로 구성한다.
  - 간격은 gap 계열 규칙을 우선 사용한다.
  - 렌더 트리는 가능한 얕게 유지한다.
  - 렌더 경로에 비즈니스 로직을 넣지 않는다.
  - UI 코드에서 unwrap 사용을 지양한다.

  ## 상호작용 및 상태 기준

  - 이벤트 처리는 listener 패턴을 사용한다.
  - 로컬 상태 변경 후 필요한 경우 cx.notify를 호출한다.
  - div 클릭 상호작용은 상태 기반 interactivity 요건을 만족한다.
  - 테마 변경은 Theme::change(ThemeMode::Light 또는 ThemeMode::Dark, Some(window), cx)로 처리한다.

  ## 플랫폼 및 안전 기준

  - 플랫폼 종속 코드는 platform 경로 하위에 분리한다.
  - Windows 종속 구현은 cfg(target_os = "windows") 게이트를 유지한다.
  - 사용자 요청 없는 파괴적 git 명령은 사용하지 않는다.
  - 관련 없는 기존 변경사항은 보존한다.

  ## 검증 및 완료 보고 기준

  - 코드 수정 후 cargo check를 수행한다.
  - 단계 완료 확인 시 cargo build를 수행한다.
  - 새로 유입된 오류는 종료 전에 해결하거나 원인을 명시한다.

  ## 작업 후 오류 리뷰 기준

  - 각 구현 작업 직후 Error Reviewer 서브 에이전트로 오류 전용 리뷰를 수행한다.
  - 리뷰 범위는 컴파일, 린트, 진단 오류로 제한한다.
  - 완료 보고에는 아래 중 하나를 반드시 포함한다.
    - 코드 오류 없음
    - 오류 목록, 위치, 원인 요약

  ## 문서 구조

  - 메인 지침: .github/copilot-instructions.md
  - 파일별 자동 적용 지침: .github/instructions/kakao-gpui-core.instructions.md
  - 오류 분석 서브 에이전트: .github/agents/error-reviewer.agent.md
  - 루트 안내 문서: AGENTS.md



