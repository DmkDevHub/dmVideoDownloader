# Chrome MV3 확장 소스 경계

루트에서 `npm ci`, `npm run build:extension` 실행 후 **`extension/dist/`**를 Chrome 개발자 모드에서 unpacked 로드한다. `extension/`은 소스 폴더이며 직접 로드 대상이 아니다. 배포 시 dist 내용 전체를 포터블 `extension/`에 복사한다(R7).

확장은 영상 후보 감지와 사용자 선택 전달을 담당하며 다운로드 엔진을 실행하지 않는다. 설치·연결 구현 전 `docs/BROWSER_EXTENSION.md`, `docs/PACKAGING.md`, `docs/SETTINGS.md`, `docs/IPC_PROTOCOL.md`를 읽는다. unpacked 수동 로드 및 정확한 확장 ID allowlist 방식을 유지한다.

TypeScript 설정은 `../shared/tsconfig.base.json`을 기반으로 한다. `npm run check`, `npm test`는 데스크톱과 확장을 함께 검사한다.

`npm run test:extension:chrome`은 실제 headless Chrome for Testing 프로세스로 검증한다. `DM_CHROME_BINARY`에 해당 실행 파일 경로를 지정한다. 임시 프로필·로컬 HTTP fixture·테스트 프로세스 전용 호스트 매핑만 사용하며 화면 클릭/입력·쿠키 조회·사용자 Chrome 프로필/정책 변경은 하지 않는다. 일반 Chrome 수동 설치·시각적 확인을 대신하지 않는다.

`DM_MEDIA_FIXTURE`에 공개 테스트 MP4의 절대경로를 지정하면 메타데이터뿐 아니라 내부 HTML5 재생 시간 증가를 확인한다. 미지정 시 재생은 NOT RUN으로 출력한다. 사용한 공개 fixture 출처·해시는 `../docs/ENVIRONMENT.md` 참조. 테스트가 자산을 자동 다운로드하지 않으며 생성물·테스트 프로필은 Git에서 제외한다.

권한은 관찰용 webRequest, 문서 ID/수명 정리용 webNavigation, 세션/옵션 캐시 storage, 사용자 선택/진단용 nativeMessaging이다. HTTP(S) 호스트 접근은 자동 감지에 필요하며 Chrome 사이트별 접근 설정을 존중한다. cookies/debugger/downloads/tabs 권한과 인증정보 수집은 없다.

MP4/WebM/HLS/DASH 후보·현재 탭 목록/배지, 명시 선택만 Host 전송, 실제 확장 ID/복사 및 진단 시작/결과를 구현했다. 앱 설정 실제 교환·Host 등록/왕복·다운로드는 아직 제공하지 않는다. 팝업의 안내를 따르더라도 현재 빌드에서는 앱 수신 성공을 기대하면 안 된다. ID/폴더 이동 후 실제 Chrome ID 확인·앱 재등록·왕복 테스트가 필요하다(R3/R7).
