# Chrome MV3 확장 소스 경계

R0-03에서 확보한 확장 전용 폴더이다. manifest·서비스 워커·content script·팝업의 실행 골격은 R2-01에서 구성한다. 현재 Chrome에 로드할 수 있는 확장이나 빌드 명령은 없다.

확장은 영상 후보 감지와 사용자 선택 전달을 담당하며 다운로드 엔진을 실행하지 않는다. 설치·연결 구현 전 `docs/BROWSER_EXTENSION.md`, `docs/PACKAGING.md`, `docs/SETTINGS.md`, `docs/IPC_PROTOCOL.md`를 읽는다. unpacked 수동 로드 및 정확한 확장 ID allowlist 방식을 유지한다.

향후 TypeScript 설정은 `../shared/tsconfig.base.json`을 기반으로 한다. 실제 확장 빌드를 추가할 때 루트 명령과 `docs/BUILD.md`를 함께 갱신한다.
