# Chrome MV3 확장 소스 경계

루트에서 `npm ci`, `npm run build:extension` 실행 후 **`extension/dist/`**를 Chrome 개발자 모드에서 unpacked 로드한다. `extension/`은 소스 폴더이며 직접 로드 대상이 아니다. 배포 시 dist 내용 전체를 포터블 `extension/`에 복사한다(R7).

확장은 영상 후보 감지와 사용자 선택 전달을 담당하며 다운로드 엔진을 실행하지 않는다. 설치·연결 구현 전 `docs/BROWSER_EXTENSION.md`, `docs/PACKAGING.md`, `docs/SETTINGS.md`, `docs/IPC_PROTOCOL.md`를 읽는다. unpacked 수동 로드 및 정확한 확장 ID allowlist 방식을 유지한다.

TypeScript 설정은 `../shared/tsconfig.base.json`을 기반으로 한다. `npm run check`, `npm test`는 데스크톱과 확장을 함께 검사한다.

`npm run test:extension:chrome`은 실제 headless Chrome for Testing 프로세스로 검증한다. `DM_CHROME_BINARY`에 해당 실행 파일 경로를 지정한다. 임시 프로필·로컬 HTTP fixture·테스트 프로세스 전용 호스트 매핑만 사용하며 화면 클릭/입력·쿠키 조회·사용자 Chrome 프로필/정책 변경은 하지 않는다. 일반 Chrome 수동 설치·시각적 확인을 대신하지 않는다.
