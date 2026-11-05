# Windows Native Messaging Host 소스 경계

R0-03에서 확보한 별도 Windows EXE용 폴더이다. 구현·Cargo manifest·빌드 명령은 아직 없으며 R3에서 추가한다. 데스크톱 앱과 같은 실행 파일로 합치지 않는다.

호스트의 책임은 Chrome stdio JSON 길이 프레이밍과 사용자별 로컬 IPC 사이의 중계이다. `stdout`은 프로토콜 전용으로 유지하며 GUI·다운로드 엔진·쿠키/인증 세션 처리 기능을 넣지 않는다.

등록·연결 구현 전에 `docs/BROWSER_EXTENSION.md`, `docs/PACKAGING.md`, `docs/SETTINGS.md`, `docs/IPC_PROTOCOL.md`를 읽는다. 현재는 HKCU 등록·해제나 Chrome 연결을 수행하지 않는다. 실제 빌드 추가 시 `docs/BUILD.md`에 명령과 산출물을 기록한다.
