# CHANGELOG

모든 개발 변경은 ROADMAP의 **세부 작업 1개 완료 단위**로 기록한다. 명세 작성과 실제 구현을 구분한다.

## 2026-11-09 — [R7-02] document and verify Chrome installation and deregistration

- 휴대용 README에 실제 설정 메뉴, 새 프로필 unpacked 로드, ID 등록·왕복 테스트·복구·해제를 설명했다. Chrome for Testing에서 릴리스 Native Host의 정확한 allowlist, 후보 전달, 왕복 ACK, 복구·해제 후 미등록 응답을 확인했다. Chrome 미등록 오류의 고정 안내와 확장 단위 테스트 30개 PASS. 사용자 화면 조작은 미검증.

## 2026-11-09 — [R7-01] build a verified Windows portable ZIP

- Windows x64 릴리스 앱·Host, 빌드된 확장, 검증된 yt-dlp/FFmpeg 및 라이선스·출처를 ZIP으로 묶었다. 도구 변조 입력을 빌드 전에 거부했다. 별도 추출 폴더에서 페이로드 15개와 ZIP SHA-256을 검증했다. 시스템 WebView2를 사용하는 미서명 로컬 검증용 패키지이다.

## 2026-11-09 — [R6-04] classify stream failures and verify HLS through the app

- 누락·빈 조각, 잘못된 범위, 전송 중단, 요청 한도와 리디렉션을 고정 메시지로 분류했다. 정상 206, 잘못된 범위, 인증성 주소 차단과 실제 공개 HTTP/DRM 응답, 503 재시도 및 401 즉시 중단을 회귀 검증했다. npm run verify 단위 80개와 숨김 WebView에서 공개 HLS 저장 및 양쪽 트랙 디코딩 PASS. 화면 시각 인수는 미검증.

## 2026-11-09 — [R6-03] group adaptive variants without losing candidate identity

- TC-06 PASS. 동일 탭의 명확한 HLS/DASH 변형을 묶고 마스터 발견 시 대표 주소만 교체한다. 식별 쿼리·경로가 다른 영상은 분리하고 CMAF·알려진 조각·별도 음성 URL을 제외한다. Chrome for Testing에서 반복 요청, 후보 ID 유지, 탭 분리와 워커 복원 확인. 확장 테스트 29개 및 실제 공개 MP4 재생 PASS.

## 2026-11-09 — [R6-02] transfer validated segmented DASH video and audio

- TC-05 전송 검증 PASS. 공개 Big Buck Bunny 정적 DASH에서 720p 영상과 분리 음성의 초기화/미디어 조각을 각각 160개 전송해 MP4로 병합했다. 출력 영상·음성의 처음 5초를 FFmpeg로 디코딩했다. Rust 33개 PASS. 보호·동적/다중 기간·외부 링크·범위 조각 거부와 주소 검증을 포함한다.

## 2026-11-09 — [R6-01] download clear HLS variants and matched audio

- TC-04 전송 검증 PASS. 공개 Shaka HLS 마스터에서 720p 상한 이하 576p 영상과 기본 음성을 선택해 조각을 받고 MP4로 병합하여 두 트랙을 디코딩했다. 보호 선언·라이브·사설/민감 주소·바이트 범위 검증, Rust 31개와 타입 검사 PASS. 브라우저 UI 조작과 시각 인수는 미검증.

## 2026-11-09 — [R5-06] enforce download concurrency and dispatch Windows notifications

- 설정의 1~4개 동시 작업 제한과 대기 작업 실행, 고정 실패 메시지, 완료 알림 설정 적용을 확인했다. Native Host·숨김 WebView 통합에서 실제 MP4 2개를 서로 다른 이름으로 저장·디코딩했으며 Windows Shell이 활성 작업의 완료 알림 요청 1건을 수락했다. npm run verify와 실제 도구 회귀 12개 PASS. Windows 알림 시각 표시와 릴리스 인수는 미검증.

## 2026-11-09 — [R5-05] manage download lifecycle and transient retries

- TC-08 작업 대기·분석·전송·병합·일시정지·취소·실패·완료 상태와 제한된 네트워크 재시도를 검증했다. 실제 전송 중 일시정지 시 임시 파일 0개, 처음부터 재개 후 저장을 확인했다. 실제 503은 설정대로 재시도하고 401은 재시도 없이 중단했다. Windows 프로세스 종료와 게시/취소 경계 테스트 PASS. 바이트 단위 이어받기는 제공하지 않는다.

## 2026-11-09 — [R5-04] publish downloads safely in the configured folder

- TC-01 실제 설정 저장 후 새 Store로 읽은 한글 폴더에 공개 MP4를 저장하고 디코딩했다. 제목·제목-주소 해시·날짜-제목 규칙, Windows 예약명·경로 문자·긴 제목, 동시 같은 이름의 번호 부여와 기존 파일 보존을 테스트했다. Cargo 검사와 다운로드 테스트 PASS.

## 2026-11-09 — [R5-03] merge validated video and audio streams with FFmpeg

- TC-05/07 엔진 검증. 실제 공개 Sintel DASH에서 720p 상한 이하 546p 영상과 별도 음성 스트림을 받아 FFmpeg로 MP4 스트림 복사 병합하고 두 트랙을 디코딩했다. 공개 단일 MP4 디코딩과 포맷 선택 등 다운로드 테스트 5개 PASS. FFmpeg는 로컬 작업 파일만 읽는다. Chrome 감지부터 재생까지의 릴리스 인수와 세그먼트 전송은 별도 범위이다.

## 2026-11-09 — [R5-02] apply best and capped video quality settings

- TC-07 선택 정책 검증. 최고 및 2160p·1080p·720p 상한, 정확히 일치하는 포맷이 없을 때 상한 이하 선택, 높이 미확인/초과만 있는 경우의 안내를 테스트했다. 실제 공개 Sintel DASH 메타데이터의 최고와 720p 상한 선택을 비교했다. 공개 MP4 저장·디코딩 회귀 PASS. 화질을 바꾼 실제 DASH 파일은 R5-03에서 검증한다.

## 2026-11-09 — [R5-01] download public progressive video and report progress

- 공개 MP4를 기존 주소 검증 후 실제 yt-dlp로 내려받고 바이트 기준 진행률을 받았다. 1,128,375바이트 파일을 FFmpeg로 끝까지 디코딩하고 임시 디렉터리 정리를 확인했다. npm run check, npm test, 데스크톱 Rust 23개 PASS. 단일 MP4/WebM 작업 기준이며 화면 시각 인수는 미검증.

## 2026-11-09 — [R4-05] classify public media analysis failures

- TC-10/11 PASS. 인증 필요·만료/제거·접근 거부·DRM 보호/의심·미지원·네트워크 오류를 분류하며 보호 및 허용되지 않은 주소의 포맷을 제외한다. 실제 HTTP 401/403/410, 공개 ClearKey DASH, 내부 주소 리디렉션 차단, 공개 HLS 11개 포맷, 숨김 WebView 결과/오류 표시를 확인했다. npm run verify 및 실제 도구 테스트 7개 PASS. HTTPS 터널 내부와 화면 시각 인수는 별도 범위이다.

## 2026-11-09 — [R4-04] normalize media dimensions and track codecs

- 포맷별 가로·세로 해상도, 컨테이너, 영상·음성 코덱과 통합·분리 트랙을 정규화한다. 결측·잘못된 값과 음성 단독 입력 처리, 중복 ID 제거 단위 테스트 PASS. 실제 공개 DASH에서 포맷 20개와 영상 전용·음성 전용 트랙 확인. 타입 검사 통과.

## 2026-11-09 — [R4-03] analyze selected candidates with a bounded media fallback

- 후보 행에서 분석을 요청하면 저장된 페이지 주소를 우선 사용한다. 미지원·포맷 없음에서만 검증된 감지 주소를 한 번 시도하고 결과에 경로를 표시한다. 실제 example.org 미지원→공개 MP4 포맷 확인과 우선 경로·타임아웃·내부 주소 분기 테스트 PASS. 타입 검사 통과.

## 2026-11-09 — [R4-02] analyze public URLs through a guarded proxy

- 공개 MDN HTML5 페이지의 MP4·WebM 포맷 2개와 직접 MP4 1개를 Windows 실제 yt-dlp로 확인했다. 직접 URL 분석과 결과 표를 연결했다. 사설 주소 CONNECT/HTTP 차단, 혼합 DNS 응답 거부, 검증한 IP로 연결, 메타데이터에서 URL·헤더 제외 테스트 PASS. 범용·HTML5 추출기를 사용하며 HTTPS 터널 내부는 해석하지 않는다.

## 2026-11-09 — [R4-01] diagnose portable media tools

- TC-13 PASS. 실행 파일 옆 tools의 두 도구를 제한 시간과 출력 상한 안에서 검사하며 누락·경로 오류·실행 실패·지원 버전을 구분한다. Windows 실제 yt-dlp 2026.10.26과 FFmpeg 9.0.2 버전 응답 확인. Rust 진단 테스트, 타입 검사와 데스크톱 Vitest 13개 통과.

## 2026-11-09 — [R3-09] verify ownership and repair across Windows registry views

- TC-19/20 PASS. 실제 WOW64 분리 키에서 양쪽 등록 충돌, 소유 뷰만 해제, 다른 복사본·별도 값 보존, 명시 복구·반복 등록 확인. 설치 위치를 옮긴 뒤 실제 Chrome 왕복과 설정 교환 재검증 및 임시 HKCU 정리 PASS. npm run verify 단위 56개·숨김 앱 4회·프로세스 검사·빌드 PASS. 별도 Windows 사용자와 조직 정책, 수동 설치·시각 인수는 미검증.

## 2026-11-09 — [R3-08] diagnose registration and connection failures

- TC-09/18/20 진단 부분 PASS. 격리 HKCU에서 매니페스트/Host 누락 구분과 매니페스트 복구, 실제 Host 프로세스에서 미등록·앱 종료·ID/프로토콜 불일치 확인. 설정 저장·재열기 후 진단 갱신, 대기 중 등록 변경 감지. Chrome 활성화·정책 여부는 관리 화면 안내로 남김. Rust 12개·Vitest 40개 PASS.

## 2026-11-09 — [R3-07] confirm the bounded connection handshake and sync detection

- TC-17 실제 Chrome↔Host↔숨김 앱 진단 왕복 PASS. 앱에서 30초 대기, 연결·요청 ID에 묶인 PONG/ACK, 포트 종료 후 현재 성공 해제와 마지막 성공 유지. 만료 경계·다른 포트·재전송·대기 중 종료는 Rust 단위 검증. 성공 후 별도 상태 요청으로 실제 앱 자동 감지 설정을 확장에 반영. Rust 12개·Vitest 40개 PASS.

## 2026-11-09 — [R3-06] display candidates delivered by the Chrome native host

- Chrome for Testing 157 실제 확장 팝업의 내부 선택 메시지→Chrome Native Host→숨김 Tauri 후보 행→ACK PASS. 운영 등록/해제 명령으로 임시 HKCU 등록 후 제거 확인. 화면 클릭·시각적 인수와 실제 다운로드는 미검증.

## 2026-11-09 — [R3-05] repair portable paths and changed extension IDs

- TC-12/18 파일·등록 부분 PASS. 공백과 한글 경로로 실제 폴더 이동 후 이전 위치 부재, 경로 불일치, 명시 복구와 ID 교체, 이전 ID 거부를 격리 HKCU에서 확인. 설정 창에 기존/현재 경로와 확장 재로드 안내. 실제 브라우저 재연결 검증은 별도.

## 2026-11-09 — [R3-04] register one extension origin and remove only owned registration

- TC-09/17/19 등록 경계 PASS. 격리 HKCU 실제 등록·읽기·타 복사본 해제 거부, 정확한 ID/절대 매니페스트 확인. Host와 앱이 각각 ID를 검증. Rust 9개·데스크톱 Vitest 13개·Host 프레임/앱 수신 PASS. 실제 Chrome 왕복은 후속 항목.

## 2026-11-09 — [R3-03] bridge the host to the Windows desktop

- 실제 Windows Native Host EXE→Named Pipe→숨김 Tauri 앱 수신과 후보 ACK PASS. 설치 폴더별 pipe, 현재 사용자 ACL·원격 접속 거부·양방향 프로세스 경로와 SID 검사·제한 시간 적용. 데스크톱 단위 8개와 기본 smoke 4개 PASS. Chrome 등록은 후속 항목.

## 2026-11-09 — [R3-02] frame native host stdin and stdout

- Rust 프로토콜/프레임 테스트 3개와 Windows Host EXE 프로세스 검증 PASS. 분할 읽기·연속 프레임·UTF-8·0/초과/잘린 길이·stdout 분리 확인. 앱 전달은 다음 항목.

## 2026-11-09 — [R3-01] validate protocol v1 at both language boundaries

- TC-09 스키마 부분 PASS. Rust/TypeScript 공통 입력 15개, 잘린 JSON·중복 키·버전·URL 경계 검증. 확장 타입 검사와 Vitest 27개 PASS.

## 2026-11-06 — [R2-07] 실제 확장 ID와 연결 테스트 UI

- 변경: 실제 runtime.id 표시·검증 후 복사/실패 안내, 연결 테스트 대기/결과·마지막 성공 시각과 현재 확인 필요 상태 분리. 단일 connectNative 포트의 프로토콜 v1 PING/PONG→CONNECTION_TEST_RESULT/ACK, 새 요청 ID·30초 총 만료·동시 테스트 차단. ACK 전에는 성공하지 않고 끝나면 포트를 닫음. 오류는 정적 안내만 표시.
- 최종 점검: content script 전송 전에도 민감 URL 차단·MIME 파라미터 제거, 자동 감지 off의 DOM 관찰 중단/재개. 비동기 큐에는 원본 응답 헤더 대신 허용 메타데이터만 캡처. 응답 제한은 UTF-8 바이트 기준이며 잘못된 오류 코드의 프로토타입 속성 접근 차단. 확장 도구 의존성을 기존 버전으로 직접 명시하고 verify에 확장 빌드 포함.
- 검증: `npm run verify` PASS(Vitest 39개·Rust 8개·숨김 앱 4회·일반 EXE/확장 빌드). `DM_MEDIA_FIXTURE` 지정 Chrome headless 내부 검증 PASS: 실제 ID/팝업/배지, MP4 실제 재생·후보, HLS/DASH/WebM 메타데이터·동적 DOM, 탭/문서 정리·워커 중단/복구·off. 복사/진단 포트 상태 전이는 내부 어댑터 테스트이며 실제 Host 호출·레지스트리 변경 없음.
- 문서: README/BUILD/확장 안내·ENVIRONMENT/TEST_PLAN·ROADMAP에 R2 전체 구현 범위와 제한 반영. TC-03/04/05/06 및 TC-09/16/17의 해당 부분 검증. OS clipboard 쓰기·사용자 Chrome 수동 설치·시각적 배치·실제 Host/앱 왕복·다운로드는 NOT RUN. 다음 후보 R3-01, 미착수. 항목별 로컬 커밋, 푸시 없음.

## 2026-11-06 — [R2-06] 현재 탭 팝업·배지·명시 선택

- 변경: 제목/사이트/유형·화질/보호 미확인 목록과 탭별 개수 배지, 빈 목록/사이트 접근·연결 설정 안내. textContent 렌더링과 제목 URL 정제. 마지막 확인 자동 감지 옵션/기본값 및 동기화 대기 표시, off 시 후보·배지 정리. 세션 쓰기 실패 표시.
- 명시 선택만 고정 Host의 프로토콜 v1 VIDEO_CANDIDATE 전송. 현재 문서/후보 재확인, 중복 선택 차단, 15초 만료, 일치 ACK만 수신 완료 표시. 오류 전문은 노출하지 않으며 수신 완료를 다운로드 완료로 표시하지 않음.
- 검증: 타입·단위 32개·확장 빌드·headless Chrome PASS. 실제 목록/배지·off·쿼리 미노출, 내부 포트에서 ACK/위조 응답/버전/크기/오류/만료, content 발신자의 선택 요청 거부·감지만으로 Host 호출 없음. 워커 종료 검증은 ServiceWorker.stopWorker의 stopped 이벤트로 강화. 실제 Host/앱 통합·클릭/시각 인수는 NOT RUN.

## 2026-11-06 — [R2-05] 세션 캐시·문서 수명과 세그먼트 정리

- 변경: 추적 쿼리/쿼리 순서 기반 중복 제거, 명백한 ts/m4s/segment/chunk/init/range 후보 제외. webNavigation으로 루트·iframe 문서 ID를 확인하고 전체/SPA/fragment 이동·탭 종료 시 정리. 단일 비동기 큐로 복구·관찰·세션 쓰기 순서를 보장. 후보는 trusted-context storage.session에만 보관하며 워커 재시작 시 현재 문서와 대조. 탭 20개·탭별 30개·전체 200개 상한.
- 검증: 타입·단위 24개·확장 빌드·실제 headless Chrome PASS. 반복/세그먼트·두 탭 분리·전체/SPA 이동·탭 종료·워커 종료/복구와 후보 ID 유지 확인(TC-06). 헤더/인증 수집·영구 URL 저장 없음. 알려지지 않은 분할/광고 패턴은 후속 사례 기반 개선 범위.

## 2026-11-06 — [R2-04] HLS/DASH 메타데이터 분류

- 변경: m3u8/mpd 경로와 HLS MIME 별칭·DASH MIME 지원. MIME 파라미터/대소문자 정규화, 쿼리 문자열의 확장자 오인과 HTML 오류 응답 거부.
- 검증: 타입 검사·단위 19개·확장 빌드·headless Chrome PASS. TC-04/05의 HLS 응답 및 확장자 없는 DASH 응답→후보 유형 확인. 재생목록 분석·저장/병합·재생은 후속 단계이며 NOT RUN.

## 2026-11-06 — [R2-03] HTML5 소스 관찰과 후보 병합

- 변경: video currentSrc/src·하위 source, 동적 추가/속성 변경/미디어 이벤트 관찰. debounce 및 스캔 상한, BFCache pause/resume 처리. content script 발신자와 입력을 검증하고 동일 페이지/URL의 네트워크 후보와 ID 유지 병합. blob/data·민감 URL 제외.
- 검증: `npm run check`, `npm test` 18개, 확장 빌드 및 headless Chrome PASS. 실제 동적 source 추가·주소/유형 변경→후보 발견. TC-03 감지 보완 부분이며 실제 영상 재생/시각 확인은 미검증.

## 2026-11-06 — [R2-02] MP4/WebM 네트워크 감지

- 변경: 관찰 전용 webRequest/HTTP(S) 호스트 권한, 응답 시작 시 GET/2xx의 URL·Content-Type 허용 목록 투영, 탭별 미분석 후보. 로그인 내장 URL·인식 가능한 토큰/서명 쿼리와 내부 IP·위험 스킴 거부. 응답 헤더 원본은 보관/전달하지 않으며 요청 헤더를 읽지 않음.
- 검증: `npm test` 17개 PASS, 확장 타입/빌드 및 headless Chrome의 MP4/WebM HTTP 응답→후보 조회 PASS. TC-03 감지 메타데이터 부분. 실제 재생·시각 확인은 NOT RUN. DNS/리다이렉트/하위 요청의 다운로드 경계는 R4에서 검증하며 확장은 URL을 재요청하지 않음.

## 2026-11-06 — [R2-01] MV3 확장 최소 실행

- 변경: extension 워크스페이스·타입 검사·Vite 빌드, MV3 워커/content script/팝업 추가. Chrome 타입 패키지 버전 고정. 빌드 결과는 `extension/dist`. 화면 조작 없는 Chrome for Testing 프로세스/CDP 테스트 실행기 추가.
- 검증: `npm run check`, `npm test`(14개), `npm run build:extension`, `npm run test:extension:chrome` PASS. 실제 워커·격리 content context·팝업 왕복. 초기 실행기의 Chrome 기본 워커 선택과 팝업 탭 발신자 판별 문제 수정 후 통과. TC-16 일부; 수동 설치·시각적 확인 미검증.
- 사용자 Chrome 프로필·레지스트리·설정 변경 없음. 감지 및 Native Host는 후속 항목. 승인된 R2 전체를 항목별 순차 진행.

## 2026-11-06 — [R0 사전 재확인] 클론 환경 의존성 복원

- 변경: 현재 작업 경로의 추적 파일 53개 존재와 Windows/Chrome/MSVC/SDK/WebView2/Rust를 재확인. npm이 없어 공식 체크섬을 검증한 Node `24.22.1` / npm `11.19.1`을 사용자 폴더에 설치하고 Node 및 기존 Rust 사용자 PATH를 보완. `npm.cmd ci`로 프로젝트 패키지 76개 복원. 소스와 npm/Cargo 잠금 파일 변경 없음.
- 자동 테스트: `npm.cmd run verify` PASS(타입 검사·Vitest 13개·숨김 실제 앱 4회·Rust 8개·일반 디버그 빌드). Rust fmt PASS. 현재 경로에서 설정 저장/재시작 복원·손상 백업/복구·미래 버전 보존과 실제 WebView/Rust IPC 확인.
- 문서: ENVIRONMENT에 별도 환경 기록 추가, ROADMAP R0에 사전 재확인 근거 추가. 기존 체크·기록 보존. 화면/Chrome·실제 다운로드·Native Messaging·릴리스 검증은 NOT RUN. yt-dlp/FFmpeg는 현재 코드의 실행 의존성이 아니므로 R4/R7 범위로 유지.
- 다음 후보: R2-01 한 개. 이번에는 개발 착수하지 않음. 로컬 기록 커밋만 생성하며 푸시 없음.
  - 최초 커밋 실패 원인인 작성자 미설정은 사용자 지정 이름/이메일을 저장소 로컬에 설정하여 해소. 전역 Git 설정 변경 없음.

## 2026-11-05 — [R1-06] 동일 설정창의 Chrome 설치·연결 UI

- 변경: BrowserConnection.vue/browserConnection.ts에 수동 unpacked 단계 안내, 관리 주소 복사, ID 입력, 폴더 이동/ID 변경 안내, 등록/테스트/복구/해제 버튼 및 확인 필요/마지막 성공 구분 구성. Rust browser.rs는 EXE 기준 배포 리소스·표준 Chrome 경로를 진단하며 폴더/Chrome 열기는 사용자 버튼에서만 시도.
- 호스트 등록·왕복 통신 구현은 R3이므로 관련 버튼은 비활성화하고 준비 중이라고 표시. 파일 존재/ID 형식/과거 성공만으로 현재 연결됨을 표시하지 않음. 레지스트리·정책·Chrome 프로필 변경 없음.
- 자동 테스트: 최종 `npm run verify` PASS(Vitest 13개, Rust 8개). 실제 숨김 앱 4회로 한글 폴더/옵션 저장→재시작 복원, 손상 백업/복구, 미래 버전 보존, 단일 설정창 마운트/기본값/취소 및 로컬 진단 확인. Rust fmt/Clippy `-D warnings`·공백 검사 PASS.
- 요구사항: TC-01 설정 복원·TC-14 내부 통합 PASS, TC-02/15/16/17은 해당 구현 부분만 검증. 실제 다운로드 적용·OS picker 조작·시각적 배치·Chrome 설치/왕복 통신은 NOT RUN.
- 문서: README/BUILD/TEST_PLAN/ENVIRONMENT/tests 안내와 ROADMAP R1 전체 완료 반영. 다음 후보는 R2-01이며 미착수. 항목별 로컬 커밋, 푸시 없음.

## 2026-11-05 — [R1-05] 설정 손상 복구와 프로세스 재시작 유지

- 변경: 손상 파일을 자동 덮어쓰지 않고 읽기전용 기본값/복구 안내 제공. 명시적 복구 시 고유 백업을 sync한 후 기본값 저장. 미래 스키마는 일반 저장·복구 모두 거부. 설정 재읽기와 안전한 오류 메시지 제공.
- 자동 테스트: `npm run verify` PASS(Vitest 10개, Rust 7개). 실제 숨김 앱 프로세스 4회 실행으로 저장/재시작/손상 복구/미래 버전 보존 검증. 외부 파일 검사로 손상 백업 바이트와 미래 파일 원본 보존 확인.
- 요구사항: TC-01/14 내부 통합 PASS. TC-15 설정 스냅샷 분리 부분 PASS, 진행 중 다운로드는 R5 미구현으로 NOT RUN. 화면 직접 조작 없음.
- 문서: ROADMAP R1-05 반영. 다음 승인 범위 R1-06 Chrome 설치·연결 UI 구성. 푸시 없음.

## 2026-11-05 — [R1-04] JSON 설정 저장·로드

- 변경: Rust settings.rs에 strict 스키마, Windows Known Folder 기본값, EXE 기준 data/settings.json 생성·로드, 임시 파일 sync/원자 교체/재읽기 확인 구현. 파일 공유 잠금·바이트 기준 낙관적 경합 검사로 중복 앱의 조용한 덮어쓰기 방지. 저장 실패 시 UI 유지.
- 자동 테스트: `npm run verify` PASS(Vitest 8개, Rust 5개). 타입/범위/알 수 없는 키/중복 키 및 ID 검사, legacy ID 누락 호환, 실제 교체 실패 시 원본 보존, 별도 Store 재로드 및 숨김 앱의 실제 저장 IPC 검증.
- 요구사항: TC-01 저장/재로드 부분 PASS. smoke는 별도 .test-output 경로를 사용하며 사용자 설정·다운로드 경로를 변경하지 않음. 프로세스 재시작·손상 복구는 R1-05. ROADMAP 반영, 푸시 없음.

## 2026-11-05 — [R1-03] Windows 폴더 선택 및 경로 검사

- 변경: Rust dialog 플러그인을 사용자 찾아보기 동작에 연결. folders.rs에서 Windows 로컬 절대경로·예약 이름·경로 순회·시스템/리파스 경로 검사 후 폴더 생성과 고유 임시 probe로 쓰기 확인. 기존 파일은 덮어쓰지 않음. JS 파일시스템 권한은 추가하지 않음.
- 자동 테스트: `npm run verify` PASS(Vitest 7개, Rust 2개, 숨김 WebView, 일반 빌드). TC-02 부분 검증: 위험 경로 거부, 한글 폴더·probe 정리·기존 파일 보존 및 picker 어댑터 취소/오류.
- 제한: 실제 Windows picker 화면 선택·ACL 거부는 미검증. 저장 연동은 다음 R1-04. ROADMAP 반영, 푸시 없음.

## 2026-11-05 — [R1-02] 단일 설정 대화상자와 편집 초안

- 변경: SettingsDialog.vue/settings.ts에 명세의 일반 옵션, 단일 모달, 초안/적용값 분리, 취소·기본값 복원 구성. Rust가 Windows 다운로드 위치를 조회해 기본 경로 결정. 저장/찾아보기는 후속 구현 전 비활성.
- 자동 테스트: `npm run verify` PASS(단위 6개, 실제 숨김 WebView에서 기본 경로 IPC 포함, 일반 디버그 빌드). 중복 열기·취소·기본값 미저장 동작 내부 모델 검사. 시각적 배치·화면 직접 조작 미검증.
- 문서: ROADMAP R1-02 완료 반영. TC-01/02/14의 저장·폴더 검증은 R1-03~05에서 수행. 다음 승인 범위 R1-03 순차 진행.

## 2026-11-05 — [R1-01] 메인 화면 배치

- 변경: App.vue/style.css에 URL 입력, 후보·다운로드 빈 목록, 연결 확인 필요 상태, 설정/폴더 버튼 배치. 미구현 동작은 비활성화하고 화질·진행률을 지어내지 않음.
- 자동 테스트: 타입 검사, Vitest 3개, 프런트엔드 빌드, 실제 숨김 Windows WebView smoke PASS. 화면 직접 조작 없음.
- 요구사항 검증: 컴포넌트 내부 렌더링과 실제 Vue 마운트 확인. 시각적 배치·제품 기능은 미검증. 직접 대응 TC 없음. ROADMAP 반영, 다음 승인 범위 R1-02 순차 진행.

## 2026-11-05 — [R0-04] 기본 내부 자동화 테스트 체계

- 변경: Vitest 실행 설정, Vue SFC 내부 렌더링 및 기본 네이티브 권한/CSP 단위 테스트 3개 추가. 루트 `npm test`, `npm run verify` 진입점과 개발 의존성 잠금 파일 갱신.
- 자동 테스트: `npm run verify` 전체 PASS (타입 검사, 2개 파일·3개 단위 테스트, 숨겨진 실제 Windows WebView/Vue/Rust 호출 및 정상 종료, 일반 디버그 빌드). Rust fmt 및 `git diff --check` PASS.
- 요구사항 검증: 화면 조작 없이 테스트 실행·실제 Windows 앱 구동을 확인하고 마지막 산출물을 일반 디버그 EXE로 복원. 직접 대응 TC 없음; TC-01~20 기능 테스트는 NOT RUN.
- 문서: README/BUILD/tests 안내, TEST_PLAN 상세 결과, ENVIRONMENT 후속 검증 및 ROADMAP R0-04/R0 전체 완료 반영.
- 남은 제한: 시각적 레이아웃·릴리스 배포·제품 기능 미검증. 다음 후보는 R1-01 하나이며 추가 사용자 지시 전 착수하지 않음. 푸시 없음.

## 2026-11-05 — [R0-03] 소스 폴더 경계 및 빌드 규칙

- 변경: `extension`, `native-host`, `shared`, `tests` 폴더의 책임/후속 구현 경계를 기록. 공통 TypeScript 설정을 `shared/tsconfig.base.json`으로 분리해 데스크톱에서 사용. `docs/BUILD.md`에 실제 빌드 명령·잠금 파일·산출물/제외 규칙 정리.
- 자동 테스트: 폴더/필수 파일·공통 설정 연결 PowerShell 검사, `npm run build:debug`, `git diff --check` PASS. npm/Cargo 잠금 파일 변경 없음.
- 요구사항 검증: 모듈 경계와 기존 Windows 앱 빌드 확인. 확장 manifest/실행은 R2, Native Host 프로토콜/EXE는 R3, 공통 메시지 타입/스키마는 R3-01에서 구현하며 현재 기능이 있다고 표시하지 않음. 직접 대응 TC 없음, TC-01~20 NOT RUN.
- 문서: README 및 ROADMAP R0-03 완료 반영. 다음은 승인된 R0-04 테스트 체계 구성.

## 2026-11-05 — [R0-02] 최소 Windows 앱 실행 골격

- 변경: `apps/desktop`에 Tauri 2 + Vue 3 + TypeScript + Rust 골격 구성. 루트 npm 워크스페이스, npm/Cargo 잠금 파일, 최소 CSP·권한 없는 설정 및 빌드 산출물 제외 규칙 추가. 영상 감지·다운로드·설정 기능은 미구현.
- 자동 테스트: `npm run check`, `npm run build:debug`, `npm run smoke`, `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`, `git diff --check` PASS.
- 요구사항 검증: Windows 11 Pro에서 숨겨진 실제 네이티브 WebView 창 생성→Vue 마운트→Rust 호출→정상 종료 확인. 화면 클릭·입력 없음. 테스트 IPC는 별도 feature/프런트엔드 모드로 한정. 직접 대응 TC 없음, TC-01~20 NOT RUN.
- 문서: README 실행 명령, ENVIRONMENT 환경 재확인 결과 및 ROADMAP R0-02 완료 반영. CIM 읽기 조회로 기존 Windows 제품명 미확정 해소.
- 남은 제한: 시각적 레이아웃·사용자 조작·릴리스 배포 미검증. 다음은 사용자 승인 범위의 R0-03을 순차 진행.

## 2026-11-05 — 새 컴퓨터의 R0 재확인 규칙 반영

- R0 완료 표시와 관계없이 새 컴퓨터에서 최초 개발 전 환경·기존 빌드·내부 자동화 테스트를 재확인하도록 AGENTS, CODEX_START, README에 명시.
- ENVIRONMENT에 환경별 결과 추가 방식 명시. 기존 소스·조사 이력·완료 체크는 보존하고 실패·미검증은 별도 기록. 후속 개발 자동 착수 없음.
- 검증: `git diff --check` 및 문서 간 정책 일치 확인. 문서 변경으로 앱 테스트는 해당 없음. 현재 컴퓨터의 R0-01 조사 이력은 이미 존재하며 다른 컴퓨터의 검증은 수행하지 않음.

## 2026-11-05 — 내부 자동화 검증 규칙 반영

- 사용자 지시에 따라 AGENTS, CODEX_START, TEST_PLAN에 화면 직접 조작 없이 내부 자동화 테스트로 검증하는 규칙 반영. 브라우저/데스크톱 클릭·입력 자동화도 제외.
- 실제 사용 인수 조건은 유지하며 내부 자동화로 확인하지 못한 조건은 미검증/보류로 기록하도록 명시.
- 검증: `git diff --check` 및 변경 문서의 정책 일치 확인. 문서 변경으로 앱 테스트는 해당 없음. ROADMAP 체크 상태 변경 및 다음 개발 작업 착수 없음.

## 2026-11-05 — 작업 완료 후 로컬 커밋 규칙 변경

- 사용자 지시에 따라 AGENTS, README, CODEX_START의 Git 규칙을 완료·검증한 작은 작업마다 로컬 커밋 생성, 푸시 금지로 통일.
- 직전 완료된 R0-01 조사 기록과 함께 로컬 커밋으로 정리. 다음 ROADMAP 항목은 착수하지 않음.
- 검증: 문서 간 Git 규칙 일치 및 `git diff --check` 확인. 문서 변경이므로 앱 테스트는 해당 없음.

## 2026-11-05 — [R0-01] 기존 프로젝트·환경·파일 조사

- 변경: `docs/ENVIRONMENT.md`에 기존 설계 문서 15개 보존 목록, 소스/설정 부재, Windows·Node·Rust·Chrome·빌드 도구 조사 결과 기록. ROADMAP의 R0-01만 완료 처리.
- 자동 검증: PowerShell 파일 수·확장자·소스/런타임 경로 부재 검사 PASS. Node/npm/Rust/Cargo 버전 및 Rust 타깃 실행 확인. 변경 대상 외 기존 문서 13개의 SHA-256 보존 비교 수행.
- 요구사항 검증: 환경·파일 조사 완료. 직접 대응 TC ID 없음. TC-01~20 NOT RUN(앱·확장·Native Host 미구현).
- 남은 제한: Windows 제품명 확정 보류(CIM 접근 불가), Tauri CLI 미설치, 실제 컴파일·창 실행 미검증. R0 단계 전체 미완료. 다음 후보는 R0-02이며 미착수.
- Git 명령, 도구 설치, 브라우저/레지스트리 변경은 수행하지 않음.

## 2026-11-05 — 설계 문서 v1.2

- 기존 v1.1의 15개 Markdown을 유지하며 Chrome 설치·연결 설계를 구체화.
- EXE 설정 창 설치 안내, Chrome 개발자 모드 unpacked 로드, 실제 확장 ID 입력 및 단일 allowlist 확정.
- EXE의 현재 사용자 HKCU 등록, 고정 Host 이름/절대경로, 연결 테스트·복구·해제 및 폴더 이동/ID 변경 재등록 규칙 추가.
- 확장에서 시작하는 PING/PONG 및 결과 확인 계약, 오류·타임아웃·상태 표시 정의. 웹스토어 강제 설치/기업 정책 변경 제외.
- PRD/아키텍처/UI/보안/개발 지시와 ROADMAP 및 TC-16~20을 정합화. 기존 구현 체크는 모두 미완료 유지.
- 설계 버전은 v1.2. IPC 버전과 설정 스키마는 1 유지, `browserExtensionId`는 누락 시 빈 값으로 호환.
- 검증 범위: 문서 링크·요구사항 반영·버전/계약 정합성 및 전체 ZIP 내용 일치 검사. 앱 구현 및 Windows/Chrome 실행 테스트는 NOT RUN.

## 2026-11-05 — 설계 문서 v1.1

- 단일 대형 명세 대신 `AGENTS.md`, `ROADMAP.md` 및 역할별 `docs/*.md`로 분리.
- Chrome 자동 감지, Windows 포터블 다운로드, JSON 설정, 비인증 콘텐츠 범위를 확정.
- **코드 구현·실행 검증은 아직 수행하지 않음.**

## 이후 구현 기록 템플릿

### YYYY-MM-DD — [R?-??] 작업 제목

- 변경: 파일/기능
- 자동 테스트: 수행 항목, 성공/실패
- 요구사항 검증: TC ID 및 실제 동작
- 남은 제한: 필요 시 기록
