# Codex 첫 실행 프롬프트

아래 텍스트를 Codex 채팅에 붙여 넣는다.

---

이 프로젝트는 Windows 11용 **브라우저 자동 감지 기반 영상 전용 포터블 다운로더**이다.

**먼저 반드시** `AGENTS.md`, `README.md`, `docs/PRD.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY_SCOPE.md`, `ROADMAP.md`를 읽고 기존 프로젝트 상태를 확인해줘. 작업에 필요한 상세 문서만 추가로 읽어줘.

규칙:
- 문서에 없는 기능을 임의로 추가하지 말 것.
- Chrome 영상 감지와 EXE 다운로드를 분리할 것.
- 설계 v1.2의 Chrome 설치는 EXE 설정 안내 + 개발자 모드 unpacked 수동 로드로 구현할 것.
- 실제 확장 ID allowlist, EXE의 HKCU 호스트 등록, 확장에서 시작하는 왕복 테스트, 폴더 이동/ID 변경 후 재등록을 따를 것. 웹스토어 강제 설치·기업 정책 변경은 하지 말 것.
- 설치·연결 작업 전 `docs/BROWSER_EXTENSION.md`, `docs/PACKAGING.md`, `docs/SETTINGS.md`, `docs/IPC_PROTOCOL.md`를 읽을 것.
- 로그인/인증 세션/쿠키/토큰 연동과 DRM 우회는 구현하지 말 것.
- **`ROADMAP.md`에서 가장 작은 미완료 작업 한 개만** 진행할 것.
- 해당 작업에 필요한 코드, 자동 테스트, 가능하면 실제 검증까지 수행할 것.
- 완료된 경우에만 ROADMAP 체크 상태를 갱신하고 CHANGELOG에 기록할 것.
- 작업 종료 후 변경 파일, 테스트 결과, 검증 결과, 미해결 사항을 보고할 것.
- `git init`, 커밋, 원격 연결, push, reset 등 Git 명령은 자동 실행하지 말 것.
- 다음 세부 작업은 내 추가 지시가 있을 때 수행할 것.

**첫 작업은 `R0-01`만** 진행해줘. 기존 소스가 있다면 삭제·재생성하지 말고 먼저 조사해줘.
