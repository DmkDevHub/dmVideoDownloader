# TEST_PLAN — 테스트와 요구사항 검증

## 1. 검증 방법 구분

**실행 정책 (2026-11-05 사용자 지시):** Codex는 화면을 직접 조작하지 않고 내부 자동화 테스트로 검증한다. 단위·통합 테스트와 프로세스·파일·IPC 기반 검증을 사용하며, 브라우저/데스크톱의 클릭·입력 자동화도 수행하지 않는다. 아래 실제 사용 시나리오는 인수 조건으로 유지하되 화면 확인이 필요한 부분은 NOT RUN/미검증으로 기록한다. 모의 객체 테스트 결과를 실제 Chrome·Windows 통합 성공으로 간주하지 않는다.

**자동 구현 테스트:** 순수 함수·스키마 검증·메시지 프레이밍·다운로드 상태 전이·파일명 처리·설정 복구를 단위/통합 테스트로 확인.

**실제 요구사항 검증:** Windows 11 + Chrome에서 영상 재생→후보 감지→선택→앱 전달→다운로드→영상 재생까지 사용자 행동으로 확인. 자동 테스트만 통과했는데 실사용이 실패하면 `완료`로 체크하지 않는다.

- 테스트 대상: 인증 없는 공개 미디어 또는 로컬 개발용 HTTP 테스트 서버.
- 사전 도구: Chrome, Windows 11, Rust, Node, Tauri 개발 환경, yt-dlp/FFmpeg.
- 실제 사이트 동작의 한계: 영상 주소 만료, 광고, 세그먼트, 플레이어의 동적 요청은 환경에 따라 재현되지 않을 수 있음.

## 2. 수용 테스트 매트릭스

| ID | 관련 기능 | 시나리오 | 기대 결과 |
|---|---|---|---|
| TC-01 | FR-07 | 설정에서 저장 폴더 변경→저장→앱 재실행 | 설정 JSON 및 UI 값 복원, 새 다운로드에 적용 |
| TC-02 | FR-07 | 상대 경로/쓰기 불가 경로 입력 | 오류 표시, 잘못된 저장 방지 |
| TC-03 | FR-01 | 공개 MP4/WebM 재생 | 현재 탭 팝업 후보가 표시됨 |
| TC-04 | FR-01/05 | 공개 HLS 마스터 재생 | m3u8 후보 감지, 적절한 변형 다운로드 및 재생 |
| TC-05 | FR-01/05 | 공개 DASH 분리 영상/음성 | 감지 및 병합된 결과가 재생됨 |
| TC-06 | FR-02 | 동일 탭 반복 재생/다수 세그먼트 요청 | 후보 중복이 과도하게 증가하지 않음 |
| TC-07 | FR-05 | 최고 화질/1080p 상한 변경 | 포맷 선정 정책과 실제 저장 결과 일치 |
| TC-08 | FR-06 | 다운로드 취소, 실패 후 재시도 | 상태·임시 파일·프로세스 정합성 유지 |
| TC-09 | FR-03/08 | Host 미등록, 확장 ID 불일치, 앱 미실행 | 오류 구분, 연결 안내 및 복구 |
| TC-10 | FR-09 | 로그인 필수 콘텐츠의 접근 실패 | 쿠키/인증 세션을 수집하지 않고 불가 안내 |
| TC-11 | FR-09 | DRM 보호/의심 콘텐츠 | 우회 없이 보호·미지원 사유 안내 |
| TC-12 | FR-08 | 포터블 폴더 이동 후 Chrome 재연결 | 재등록으로 정상 사용 가능 |
| TC-13 | FR-04/05 | yt-dlp/FFmpeg 누락 또는 버전 불일치 | 진단 메시지 표시, 무한 재시도 없음 |
| TC-14 | FR-07 | 설정 JSON 손상·알 수 없는 스키마 | 무단 덮어쓰기 없이 복구·안내 |
| TC-15 | FR-06/07 | 다운로드 진행 중 설정 변경 | 기존 작업 스냅샷 유지, 새 작업만 변경 |
| TC-16 | FR-07/08 | 새 Chrome 프로필에서 EXE 안내→개발자 모드→unpacked 로드→ID 입력→HKCU 등록 | 관리자/웹스토어 없이 실제 폴더 확장 로드, 확장·등록 상태 분리 |
| TC-17 | FR-03/08 | EXE 테스트 대기→팝업 테스트→PING/PONG→결과 확인/ACK; 미응답·앱 종료·버전 불일치 | 왕복 성공만 연결됨, 30초 만료/실패 표시, 진단으로 앱·다운로드 자동 실행 없음 |
| TC-18 | FR-08 | 공백/한글 경로로 전체 폴더 이동, 옛 경로 제거, 확장 재로드 및 실제 ID 재확인 | 새 절대경로 재등록, ID 변경 시 allowlist 대체 후 재연결 |
| TC-19 | FR-03/08 | 빈/잘못된 ID, 다른 확장 origin, 반복 등록, 두 복사본 등록/해제 | 정확한 ID만 허용, 자동 ID 추가 없음, 중복·타 복사본 등록 삭제 없음 |
| TC-20 | FR-07/08 | 호스트 누락·비활성 확장·정책 차단·등록 쓰기 실패·낡은 레지스트리 뷰·연결 해제 | 안전한 오류/복구 안내, 허위 성공 없음, HKLM/기업 정책 변경 없음 |

## 3. 자동화 테스트 목록

- `SettingsSchema`: 누락 키, 범위 위반, 버전 오류, 저장·로드/원자적 교체.
- `CandidateDetector`: 확장자·MIME 인식, 탭별 분리, m3u8/mpd 분류, 세그먼트·중복 제거.
- `MessageProtocol`: JSON 스키마, 길이 프레이밍, 버전, 잘못된 타입·오버사이즈, 중복 요청 ID.
- `JobState`: 큐 동시성, 작업 전이, 실패·재시도·취소·설정 스냅샷.
- `FileSafety`: Windows 금지 문자, 예약 이름, 충돌 회피, 경로 순회, 의도치 않은 스킴 거부.
- `SecretHandling`: 요청/응답 모델에 인증 필드가 없음, 로그의 URL/쿼리 마스킹.

테스트 명령은 구현된 실제 프로젝트에 맞게 `README.md`에 기록한다. 처음부터 없는 테스트 스크립트가 있다고 주장하지 않는다.

## 4. 단계별 완료 기준

| 단계 | 완료를 위한 증거 |
|---|---|
| R0 | Windows에서 최소 앱이 실행되고 개발 빌드가 통과 |
| R1 | 설정 창 조작과 settings.json 저장/복원, TC-01/02/14 |
| R2 | Chrome 실제 영상 후보/배지 확인, TC-03/04/05 감지 부분/06 |
| R3 | 후보가 Native Messaging으로 앱에 도달, TC-09/12 |
| R4 | 공개 URL의 포맷 조회와 분류된 실패 메시지 |
| R5 | 다운받은 영상이 실제 재생됨, TC-07/08/13 |
| R6 | 공개 HLS/DASH 스트림 저장/병합, TC-04/05 |
| R7 | 다른 폴더에서 포터블 실행, 등록/해제/재등록 및 회귀 테스트 |
| R8 | 실재 감지 실패 사례의 원인 분석 및 명확한 개선(조건부) |

## 5. 테스트 실행 결과 기록 양식

```text
날짜/환경: YYYY-MM-DD / Windows 11 / Chrome 버전 / yt-dlp 버전
작업: R?-??
테스트: TC-?? 또는 자동 테스트명
실행 방법: 실제 수행한 명령 또는 조작 순서
기대 결과:
실제 결과: PASS / FAIL / NOT RUN
증거: 로그(민감값 제거), 스크린샷 또는 결과 경로
미해결 사항:
```

**실행하지 않은 테스트는 `PASS`로 표시하지 않는다.** 실패/미실행 사유를 ROADMAP 항목 아래 기록한다.

## 6. 설치·연결 상세 검증 (v1.2)

- TC-16: `extension/manifest.json` 및 빌드 리소스 동봉 확인. 내부 주소 열기 실패 시 복사 안내 확인. Chrome 미설치/확장 미설치에서는 설치 완료를 단정하지 않는다.
- TC-17: 요청 ID/연결별 테스트 순서를 확인한다. 지연된 이전 테스트 결과·임의 ID·앱까지 전달하지 않은 Host PONG은 성공 처리하지 않는다. 연결 종료 후 현재 상태 무효화와 마지막 성공 시각 표시를 확인한다.
- TC-18: 확장 ID가 유지되는 경우와 달라지는 경우를 각각 확인한다. 변경된 ID로 호스트를 갱신하면 옛 ID가 허용되지 않아야 한다.
- TC-19: 이 복사본 연결 해제로 다른 경로의 등록·설정·다운로드가 지워지지 않는지 확인한다. 일반 설정 취소는 이미 실행한 등록을 되돌리지 않으며 ID 폼 취소만으로 등록이 바뀌지 않는다.
- TC-20: HKCU 등록 값과 JSON 경로를 읽어 검증하고 실패 시 재시도가 가능해야 한다. 32/64비트 뷰의 오래된 등록 진단과 타 앱 보호를 확인한다. 확장 비활성·사이트 접근 제한·호스트 연결 실패를 무리하게 동일 원인으로 단정하지 않는다.
- 자동화 대상 추가: ID 정규식/origin 생성, 현재 경로 매니페스트, HKCU 등록 어댑터의 반복 호출/소유권·실패 처리, 테스트 핸드셰이크/시간 초과. 실제 Chrome 설치·레지스트리 동작은 Windows 실기 검증으로 보완한다.

설계 문서 v1.2 작성 시 앱 소스가 없으므로 위 앱 기능 테스트는 모두 **NOT RUN**이다. 문서/ZIP 무결성 검사는 실행 검증을 대신하지 않는다.

## 7. R0 기반 검증 실행 결과

- 날짜/환경: 2026-11-05 / Windows 11 Pro x64 `10.0.26300` / Node `24.12.0`, Rust `1.99.1`. 상세 환경은 `ENVIRONMENT.md` 참조.
- 작업: R0-02~04. 기존 TC-01~20에 직접 대응하는 기반 테스트 ID가 없으므로 아래 자동 테스트명으로 기록한다.

| 실행 명령/자동 테스트명 | 실제 결과 | 확인 범위 |
|---|---|---|
| `npm run check` | PASS | Vue/TypeScript 및 테스트 설정 타입 검사 |
| PowerShell 폴더/공유 설정 검사 | PASS | 앱·확장·Native Host·shared·scripts·tests 경계와 공유 TS 설정 연결 |
| `npm test` / App startup render | PASS | 실제 Vue SFC를 내부 렌더러로 처리, 루트/제목 렌더링 및 해당 렌더링 중 fetch 호출 없음 |
| `npm test` / scaffold capabilities | PASS | 기본 창 1개, 외부 URL 없음, 명시적 네이티브 권한 없음 |
| `npm test` / production CSP | PASS | 기본 리소스/스크립트 self 제한 및 연결 대상이 로컬 Tauri IPC로 제한됨 |
| `npm run smoke` | PASS | 실제 숨김 네이티브 WebView 창 핸들, Vue 마운트, 프런트엔드→Rust 호출, 정상 종료. 30초 앱 대기/45초 프로세스 제한 |
| `npm run build:debug` | PASS | 일반 프런트엔드 및 MSVC Windows EXE 빌드, 테스트 feature 미포함 |
| `npm run verify` | PASS | 타입 검사→3개 단위 테스트→네이티브 smoke→일반 디버그 빌드 전체 순차 통과 |
| Rust fmt 및 `git diff --check` | PASS | Rust 포맷 및 변경 공백 검사 |

앱 EXE 증거 경로: `apps/desktop/src-tauri/target/debug/dm-video-desktop.exe` (생성 산출물, 커밋 제외). smoke 성공 출력: `SMOKE_PASS: native window, Vue mount, frontend-to-Rust IPC`.

화면 클릭·입력 자동화는 실행하지 않았다. Vue 내부 렌더링은 브라우저 마운트를 대신하지 않으며 실제 마운트는 별도 네이티브 smoke에서 검증했다. 화면 배치·사용자 조작·릴리스 배포 및 TC-01~20 제품 기능 검증은 NOT RUN이다. R1 이후 작업은 착수하지 않았다.

## 8. R1 메인 화면·설정 검증 결과

날짜/환경: 2026-11-05 / 앞선 ENVIRONMENT의 Windows 11 Pro 개발 컴퓨터. R1-01~06을 순차 구현했으며 R2 이후는 미착수이다. 화면 클릭·키 입력 자동화, 실제 OS picker·Chrome·탐색기 실행은 수행하지 않았다.

- `npm run verify`: 최종 PASS. 타입 검사 → Vitest 4개 파일·13개 테스트 → 실제 숨김 Windows 앱 4회 → Rust 8개 테스트 → 일반 디버그 EXE 빌드.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`: PASS.
- `cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings`: PASS.
- `git diff --check`: PASS.

| 검증 ID/항목 | 실행 방법과 결과 | 남은 범위 |
|---|---|---|
| R1-01 메인 화면 | 내부 렌더링으로 URL·후보/작업 목록·확인 필요 상태 확인, 실제 WebView 마운트 PASS | 시각적 배치·분석/감지/다운로드 미검증 |
| R1-02 설정창 | 모델 열기/중복 열기/취소/기본값/실패 저장, 내부 렌더링으로 옵션 확인. 실제 숨김 WebView에서 모델 호출→단일 모달 마운트→기본값→취소/제거 PASS | 실제 포인터/키보드 UX 미검증 |
| TC-01 | 숨김 앱에서 한글 저장 경로·화질·자동 감지 옵션 변경/저장→프로세스 종료→새 프로세스 로드, UI 모델과 디스크 값 일치 PASS | 새 다운로드에 실제 적용은 R5 미구현으로 NOT RUN |
| TC-02 | 상대/UNC/장치/예약 이름/경로 순회/보호 폴더 거부, 한글 폴더 생성·쓰기 probe 정리·기존 파일 보존 PASS. picker 어댑터의 성공/취소/오류 검사 PASS | 실제 OS picker 선택·ACL 기반 디렉터리 거부는 NOT RUN |
| TC-14 | 잘못된 JSON·타입·범위·중복/추가 필드 검사, 손상 원본 자동 덮어쓰기 거부, 명시적 백업 후 복구, 미래 스키마 저장/복구 거부 PASS. 실제 앱 단계와 파일 바이트 검사로 보완 | 화면 조작 없음 |
| TC-15 | 저장 후 새 적용 객체 생성, 이전 스냅샷·미저장 초안 분리 PASS | 진행 중 다운로드 유지 시나리오는 R5 미구현으로 NOT RUN |
| TC-16 | 같은 설정창 안의 수동 개발자 모드/unpacked/실제 ID/등록/양쪽 테스트 안내, 리소스 진단 PASS | Chrome 최초 설치·수동 로드는 R2/R3/R7에서 검증, 현재 NOT RUN |
| TC-17 | 실제 왕복 근거 없는 연결됨 방지, 마지막 성공과 현재 상태 분리, 미구현 연결 작업 비활성 UI PASS | Native Messaging 왕복·30초 실제 대기/만료는 R3 미구현으로 NOT RUN |
| TC-18~20 | ID 형식·ID 변경/폴더 이동 안내, 리소스 존재와 설치/연결 성공 구별 PASS | 실제 HKCU 등록·소유권 해제·다중 복사본/뷰·정책/이동 검증 NOT RUN |

숨김 통합 테스트는 별도 `.test-output/smoke-*`만 사용한다. `write`, `restart`, `corrupt`, `future` 단계마다 종료 코드 0과 `SMOKE_PASS`를 확인하고, 한글 저장 경로·손상 백업 바이트·미래 파일 원본은 실행기에서 별도 읽어 검사한다. 테스트 전용 IPC/경로 주입은 일반 빌드에 포함되지 않는다. 사용자 설정·다운로드 폴더·레지스트리와 Chrome 프로필을 변경하지 않았다.

현재 R1 완료는 메인/설정 기능과 Chrome 설치·연결 **UI 구성** 범위이다. 확장 감지, 호스트 등록·연결, 실제 다운로드 성공을 뜻하지 않는다.

## 9. R2 Chrome 확장 검증 (2026-11-06)

환경: Windows 11 Pro, Node 24.22.1 / Chrome for Testing 157.0.8103.38. 실행기는 `scripts/extension-smoke.mjs`; `DM_CHROME_BINARY` 지정 후 `npm run build:extension`, `npm run test:extension:chrome`. 임시 프로필과 테스트용 loopback HTTP fixture/호스트 매핑 사용. 화면 클릭·입력·사용자 Chrome 프로필·정책·레지스트리 변경 없음. CDP는 테스트 실행 컨텍스트/함수/DOM 상태 검사에만 사용하고 제품 확장은 debugger 권한을 갖지 않는다.

- R2-01 / TC-16 일부: 실제 unpacked MV3 워커 로드, HTML 페이지의 격리 content script 컨텍스트, 팝업 로드와 워커 왕복 PASS. 타입 검사 및 단위 14개 PASS. 일반 Chrome 개발자 모드 수동 로드·시각적 배치 NOT RUN.
- R2-02 / TC-03 메타데이터: `npm test` 17개 및 확장 타입/빌드 PASS. 실제 Chrome HTTP fixture의 MP4/WebM 응답을 Content-Type/확장자로 감지하고 올바른 탭·미분석 상태 조회 PASS. 206·비GET·403·배경 요청·인증 쿼리·정수/16진 loopback·IPv6/사설 IP·헤더 미보관 단위 검사. fixture 응답은 메타데이터 전용이며 실제 영상 재생은 NOT RUN.
- R2-03 / TC-03 보완: 타입·단위 18개·빌드·headless Chrome PASS. 페이지에 video/source를 동적으로 추가하고 src/type을 바꾸어 실제 content script→워커 후보 확인. 동일 URL의 DOM/네트워크 ID 유지 병합, blob/민감 URL/잘못된 MIME 거부 단위 검사. 실제 재생/화면 확인 NOT RUN.
- R2-04 / TC-04/05 감지 부분: 타입·단위 19개·빌드·headless Chrome PASS. HLS 응답 및 확장자 없는 DASH 응답 분류, HLS MIME 별칭/대소문자/파라미터·오류 HTML·쿼리 확장자 오인 거부 검사. 재생목록 분석·품질 선택·다운로드·병합/재생 NOT RUN.
- R2-05 / TC-06: 타입·단위 24개·빌드·headless Chrome PASS. 실제 반복 MP4/추적 쿼리·chunk/segment/range 요청 후 후보 수 유지. 두 탭 분리·전체 페이지/SPA 이동·탭 종료·워커 강제 종료 후 세션 복구/후보 ID 유지 확인. 이전 root/iframe 문서 거부·상한·불량 세션 입력·민감 URL 제거 단위 검사. 원본 URL은 session에만 일시 보관하며 local/sync 저장소에 쓰지 않는다. 화면 확인/실제 재생 NOT RUN.
- R2-06 / TC-06/09 관련: 타입·단위 32개·빌드·headless Chrome PASS. 현재 탭 팝업 DOM/배지 일치·URL 쿼리 미노출·확인값 off에 따른 후보/배지 정리 확인. 포트/컨트롤러 내부 테스트에서 감지/조회/위조 content 발신자는 Host 호출 0회, 명시 선택 1회, stale/중복 선택·일치하지 않는 ACK·버전·크기·raw 오류·15초 만료 거부. 서비스 워커는 ServiceWorker.stopWorker 및 stopped 이벤트로 실제 중단 확인 후 복구. 실제 Host/앱 왕복·다운로드·UI 클릭/시각 배치는 NOT RUN.
- R2-07 / TC-17 확장 측 부분: `npm run verify` PASS(Vitest 데스크톱 13개+확장 26개, Rust 8개, 숨김 앱 4회, 일반 EXE·확장 빌드). 실제 Chrome runtime.id와 표시 ID 일치·테스트 버튼/현재 확인 필요 상태 PASS. 포트 어댑터에서 단일 포트/새 ID/일치 PONG→결과 확인→ACK만 성공, PONG 단독·선행/지연 ACK·ID/버전/UTF-8 크기 오류·앱 접근 불가·동시 진단 거부·30초 총 만료 PASS. ID 복사 어댑터 성공/실패 PASS. 원본 오류 문자열/URL을 UI에 재출력하지 않음.
- 최종 TC-03 보완: `DM_MEDIA_FIXTURE=.test-output/flower.mp4`(실행 시 절대경로 지정)로 공개 CC0 MP4를 로컬 fixture에 공급. Chrome HTMLVideoElement.play/현재 재생시간 증가·MP4 후보와 팝업 표시 PASS. 스크린·클릭·입력은 사용하지 않음. WebM/HLS/DASH는 실제 HTTP 메타데이터 감지까지이며 해당 영상 재생·분석/다운로드·병합은 NOT RUN. fixture 출처/해시는 ENVIRONMENT 참조. 미디어 fixture 미지정 시 메타데이터 검증은 실행하되 재생은 명시적으로 NOT RUN 출력.
- 최종 경계 보완: DOM 출발 시 민감 URL/내부 IP를 메시지 전송 전에 차단, MIME의 부가 데이터 미전달, off 정책의 DOM 관찰 중단/재개. 후보는 session, 마지막 비밀값 없는 옵션만 local에 유지. 설정의 실제 앱↔확장 교환은 R3 이전이므로 미검증/동기화 대기.

R2 완료는 확장 측 기능과 위 내부·실제 headless 프로세스 검증 범위이다. OS clipboard 실제 쓰기·일반 Chrome 관리 화면의 수동 설치·시각적/포인터/키보드 인수, 실제 Host/EXE 왕복·등록/다운로드 및 포터블 배포는 미검증으로 유지한다. 전체 릴리스 TC-01~20 통과를 뜻하지 않는다.

## 10. R3 브라우저와 Windows 연결 검증 (2026-11-09)

| 검증 범위 | 실행 결과 |
|---|---|
| TC-09 메시지와 Host 오류 | 공통 v1 입력 18개 및 분할·연속 프레임 테스트 PASS. 실제 Host 프로세스의 미등록·ID 불일치·앱 종료·버전 오류 응답과 stdout 프레임 분리 PASS |
| TC-09 후보 전달과 접근 제어 | 실제 Host와 숨김 앱 사이 Named Pipe PASS. 같은 사용자로 실행한 Node 프로세스의 요청 거부, 같은 후보 요청 ID의 반복 ACK 및 앱 목록 1행 확인 |
| TC-16/17 자동화 가능 범위 | 실제 Chrome for Testing 임시 프로필에서 확장 로드, 앱 명령을 통한 HKCU 등록, 팝업 메시지→Host→앱 후보 행·ACK PASS. 앱 대기 후 실제 연결 테스트·포트 종료·마지막 성공 기록·자동 감지 설정 교환 PASS. 30초 경계, 다른 포트/재전송과 대기 중 종료는 Rust 단위 테스트 PASS |
| TC-12/18 이동과 복구 | 공백·한글 경로 이동, 새 ID로 등록 교체, 이전 ID 거부는 실제 파일·격리 HKCU 테스트 PASS. 실제 Chrome 통합은 앱 폴더를 공백 경로로 옮기고 이전 키가 남은 상태에서 복구한 뒤 후보 전달·진단을 다시 수행해 PASS |
| TC-19/20 등록 소유권 | 분리된 WOW64 테스트 뷰에 서로 다른 복사본을 등록해 불일치 검출, 소유 뷰 해제, 다른 복사본과 별도 레지스트리 값 보존, 명시 복구와 반복 등록 PASS. 실제 Chrome 통합 뒤 임시 기본값 제거 확인 |
| 기존 기능 회귀 | npm run verify PASS. Vitest 40개, 데스크톱 Rust 13개, 프로토콜 Rust 3개, 숨김 데스크톱 스모크 4회, Host 프레임과 앱 IPC 프로세스 검사, 일반 EXE/Host/확장 빌드 |

일반 Chrome의 관리 화면에서 수동 설치·클릭·클립보드·시각 인수, 확장 폴더 자체를 옮겨 실제 새 Chrome ID로 다시 로드, 다른 Windows 사용자 계정으로의 ACL 시도, 조직 정책에 의한 실제 차단과 별도 일반 사용자 계정의 권한 거부는 NOT RUN이다. 전체 다운로드·재생 릴리스 인수는 R4 이후에 진행한다.

## 11. R4 공개 영상 분석 검증 (2026-11-09)

| 검증 범위 | 실행 결과 |
|---|---|
| TC-13 도구 진단 | 누락·오류·버전 범위 단위 테스트와 실제 Windows yt-dlp 2026.10.26·FFmpeg 9.0.2 호출 PASS |
| 공개 페이지 및 후보 분석 | MDN HTML5 페이지의 MP4·WebM 2개, 후보 페이지 미지원 후 CC0 MP4 대체 분석 PASS. 실제 Native Host로 받은 후보와 직접 입력 분석 결과가 숨김 WebView의 포맷 표에 표시됨 |
| TC-04/05 HLS·DASH 분석 부분 | 공개 Shaka HLS에서 11개 포맷과 복수 해상도, DASH에서 20개 포맷과 영상 전용·음성 전용 트랙 PASS. 중복 ID, 해상도·코덱 누락, 음성만 있는 입력 단위 테스트 PASS |
| TC-10과 URL 실패 | 실제 공개 HTTP 401·403·410 응답을 각각 인증 필요·접근 거부·만료 또는 제거로 분류 PASS. 원본 주소 및 헤더가 포맷 결과에 포함되지 않음 |
| TC-11 보호 감지 | 공개 Shaka ClearKey DASH의 보호 판정 PASS. 모의 보호 의심·혼합 포맷과 인증·DRM에서 대체 주소를 시도하지 않는 조건 PASS |
| 내부 주소 차단 | HTTP·HTTPS CONNECT 사설 주소 차단, 공인·사설 혼합 DNS 결과 거부, 실제 yt-dlp가 HTTP 리디렉션을 따라 로컬 서버에 재접속하지 못하는 테스트 PASS. 숨김 WebView의 고정 차단 문구 확인 |

검증된 도구를 둔 포터블 루트의 절대경로를 `DM_R4_TOOL_ROOT`에 지정한다. `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib analysis:: -- --ignored --nocapture --test-threads=1`은 공개 응답과 실제 도구를 검증한다. 같은 환경에서 `npm run verify`를 실행하면 Native Host 스모크에 숨김 WebView 분석 검증이 추가된다.

화면 시각 인수, 사이트별 지원 범위, 다운로드와 병합·재생은 이 검증 범위 밖이다.

## 12. R5 다운로드 엔진 검증 (2026-11-09)

| 검증 범위 | 실행 결과 |
|---|---|
| 공개 단일 MP4 | yt-dlp의 바이트 진행률, 지정 위치 저장, FFmpeg 전체 영상 디코딩 PASS |
| TC-07 화질 | 최고·2160p·1080p·720p 상한 단위 테스트 PASS. 공개 Sintel DASH의 최고 1636p와 720p 상한 546p를 비교하고 546p MP4를 실제 저장 |
| TC-05 분리 스트림 | 공개 Sintel DASH의 독립 영상·음성을 받고 FFmpeg 스트림 복사로 병합한 MP4에서 두 트랙 디코딩 PASS |
| TC-01/02 파일 저장 | 저장 후 새 설정 인스턴스에서 한글 경로를 불러와 실제 MP4 생성 PASS. Windows 예약명과 경로 검증, 동시 파일명 충돌 시 기존 파일 보존 PASS |
| TC-08 취소 및 재시도 | 실제 Windows 프로세스 종료, 실제 전송 중 일시정지 시 작업 파일 제거와 처음부터 재개, 503 재시도 및 401 즉시 중단 PASS. 게시와 취소의 경계 및 작업 상태 전이 단위 테스트 PASS |
| TC-13 도구와 실행 오류 | 기존 실제 도구 버전과 누락 검증, 고정 오류 분류 및 표시 경로를 회귀 검증 |
| 작업 제한과 완료 알림 | 작업 슬롯 2개와 후속 대기 작업 시작, 성공·실패·알림 설정에 따른 요청 여부 단위 테스트 PASS. Native Host→숨김 WebView→다운로드 2건과 고유 출력 파일 PASS. Windows Shell이 알림을 켠 작업의 요청 1건을 수락 |

`DM_R4_TOOL_ROOT`에 검증된 도구의 포터블 루트를 지정한다. `npm run verify`는 숨김 WebView에서 실제 다운로드를 검사하고, `DM_R5_TEST_WINDOWS_NOTIFICATION=1`을 추가하면 Windows Shell 호출의 반환값을 확인한다. `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib -- --ignored --nocapture --test-threads=1`은 공개 미디어 전송·병합·재시도와 R4 실제 도구 회귀를 실행한다.

Windows 알림의 시각 표시, 사용자 클릭·키보드 인수, Chrome 감지부터 전체 다운로드·재생까지의 릴리스 인수는 별도 검증 범위이다. 세그먼트 기반 HLS/DASH 다운로드는 R6에서 다룬다.

## 13. R6 HLS·DASH 및 후보 그룹화 검증 (2026-11-09)

| 검증 범위 | 실행 결과 |
|---|---|
| TC-04 공개 HLS | Shaka 공개 마스터에서 720p 상한 이하 576p 변형과 기본 음성을 선택해 MP4로 병합하고 두 트랙 디코딩 PASS. HLS 키 선언, 라이브, 위험 주소 거부와 초기화·바이트 범위 처리 테스트 PASS |
| TC-05 공개 조각형 DASH | 공개 Big Buck Bunny MPD의 720p 영상과 별도 음성을 각각 160개 조각 전송해 병합. 출력 양쪽 트랙의 처음 5초 디코딩 PASS. 보호·라이브·다중 기간·외부 링크 및 위험 조각 주소 거부 테스트 PASS |
| TC-06 대표 후보 | Chrome for Testing에서 변형을 먼저 보고 뒤늦게 받은 마스터로 대표 주소 교체, ID 유지, 별도 콘텐츠·탭 보존, 반복 요청 및 조각 제외, 워커 재시작 후 복원 PASS |
| 실패 회귀 | 기존 공개 HTTP 401/403/410/503과 보호 응답을 기준으로 진단을 유지했다. 로컬 HTTP 응답으로 누락·빈 조각, 전송 중 끊김, 잘못된 범위, 429, 인증성 쿼리·내부 주소 리디렉션을 재현하고 고정 오류 및 정상 206 응답 처리 PASS |

`DM_R4_TOOL_ROOT`를 설정한 `npm run verify`는 숨김 WebView의 HLS 작업 요청, Windows 저장 및 FFmpeg 양쪽 트랙 디코딩을 추가 검증한다. `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib real_segmented_dash -- --ignored --nocapture`는 공개 DASH 조각 전송을 재현한다. `DM_MEDIA_FIXTURE`에 기존 공개 MP4를 지정한 `npm run test:extension:chrome`은 실제 브라우저 감지·그룹화와 MP4 재생을 검사한다.

실제 Chrome에서 사용자가 HLS/DASH를 재생하고 팝업을 클릭하는 전체 릴리스 인수와 화면 시각 확인은 미검증이다. 일반적인 MPEG-TS/AAC 재생목록 및 실제 CDN의 바이트 범위 HLS는 위 공개 fMP4 샘플과 별도 조합이다.

## 14. R7 포터블 릴리스 검증 기록 (2026-11-09)

Windows x64에서 압축 해제한 릴리스 앱과 Native Host를 사용했다. Chrome 통합은 Chrome for Testing의 새 임시 프로필, 데스크톱 기능 통합은 테스트 명령만 추가한 숨김 WebView를 사용했다. 화면·키보드 조작과 조직 정책 변경은 테스트에 사용하지 않았다.

| ID | 확인 결과 | 남은 인수 조건 |
|---|---|---|
| TC-01 | PASS. 설정 저장·재실행, 한글 경로 로드 및 실제 공개 MP4 저장 확인 | 실제 Windows 폴더 선택창 조작 |
| TC-02 | 부분 확인. 상대·UNC·장치·보호 경로 거부와 쓰기 확인 테스트 통과 | 실제 리파스 지점과 별도 계정에서 ACL이 쓰기를 거부하는 폴더 |
| TC-03 | 부분 확인. Chrome에서 공개 CC0 MP4 재생 시간 증가와 후보 표시 확인. WebM 요청의 후보 감지 확인 | 별도 공개 WebM 재생 |
| TC-04 | 부분 확인. Chrome의 HLS 요청 감지와 숨김 앱의 공개 HLS 저장·두 트랙 디코딩 확인 | Chrome에서 같은 HLS의 재생부터 팝업 선택·저장까지의 연속 실사용 |
| TC-05 | 부분 확인. Chrome의 DASH MIME 감지와 공개 720p DASH의 영상·음성 조각 다운로드 및 병합 결과 디코딩 확인 | Chrome에서 같은 DASH의 재생부터 팝업 선택·저장까지의 연속 실사용 |
| TC-06 | PASS. 반복 요청과 조각 제외, HLS 대표 주소·후보 ID 유지, 탭 분리·워커 복원 확인 | |
| TC-07 | PASS. 최고·2160p·1080p·720p 정책 테스트와 실제 720p 상한 저장 확인 | |
| TC-08 | PASS. Windows 프로세스 종료, 취소와 파일 게시 경쟁, 실제 전송 일시정지·정리·처음부터 재개, 503 재시도·401 중단 확인 | 바이트 이어받기는 제품 지원 범위 밖 |
| TC-09 | PASS. 실제 Chrome 미등록 응답, Native Host의 ID 불일치·앱 미실행 오류 및 복구 확인 | |
| TC-10 | PASS. 공개 HTTP 401 응답을 인증 필요로 표시하고 재시도 중단 확인 | |
| TC-11 | PASS. 공개 ClearKey DASH 보호 감지와 HLS 키 선언·보호 의심 거부 확인 | |
| TC-12 | 부분 확인. 실제 릴리스 EXE가 비승격 권한에서 한글·공백 경로 이동 전후 기동하고 설정을 보존. 실제 Chrome 등록 복구·왕복 확인 | 별도 표준 사용자 계정 |
| TC-13 | PASS. 동봉 도구 버전, 누락·오류·지원 범위 검사 및 변조 입력 거부 확인 | |
| TC-14 | PASS. 손상 원본의 백업·복구 및 미래 스키마 보존 확인 | |
| TC-15 | PASS. 진행 중 MP4는 기존 폴더·컨테이너에 저장되고 변경 뒤 새 HLS는 다른 폴더에 576p MKV로 저장됨 | |
| TC-16 | 부분 확인. 실제 Chrome for Testing 새 프로필의 unpacked 로드, 실제 ID, HKCU 등록 및 안내 DOM 확인 | 일반 Chrome의 개발자 모드 토글과 폴더 선택 |
| TC-17 | PASS. 실제 Chrome·Host·앱 왕복과 미등록/종료 오류. 제한 시간·버전/포트 일치 검증 통과 | |
| TC-18 | 부분 확인. 공백·한글 경로로 앱 이동과 등록 복구. 새 Chrome for Testing 프로필에서 확장 폴더 이동으로 실제 ID 변경, 이전 allowlist 거부 및 새 ID 재등록·왕복 확인 | 일반 Chrome에서 이전 항목 제거와 새 경로 수동 로드 |
| TC-19 | PASS. 정확한 ID 한 개, 잘못된 ID 거부, 반복 등록과 타 복사본 보호 확인 | |
| TC-20 | 부분 확인. 누락·미등록·낡은 뷰·복구와 해제, 실제 접근 거부에 대한 ID·정책 안내 확인. Chrome 정책 레지스트리 전후 값 일치 | 실제 조직 정책 차단 및 별도 계정의 등록 쓰기 거부 |

Windows 알림의 화면 표시, 코드 서명과 SmartScreen 상호작용, 수동 Chrome 설치·재생의 시각적 인수는 미검증이다.

## 15. R8 고급 감지 검증 (2026-11-10)

| 재현 사례 | 기존 동작과 원인 |
|---|---|
| 중첩된 열린 Shadow DOM의 공개 MP4 `video/source`, `preload=none` | 실제 Chrome 테스트에서 후보 증가 0개, 해당 영상 요청 0건. 문서 범위의 `querySelectorAll('video')`는 Shadow DOM 경계를 넘지 않아 네트워크 관찰과 DOM 관찰 양쪽에서 놓친다. |
| 닫힌 Shadow DOM의 같은 구조 | 후보 증가 0개. 브라우저의 공개 `shadowRoot` 접근으로는 내부를 읽을 수 없다. |
| `blob:` 주소 및 인증용 쿼리가 포함된 주소 | 후보 증가 0개. 공개 주소만 처리하는 제품 경계에 따른 제외다. |

Chrome for Testing의 임시 프로필과 로컬 테스트 페이지로 재현했다. 특정 운영 사이트의 누락률은 측정하지 않았다. [Shadow DOM 접근 규칙](https://developer.mozilla.org/en-US/docs/Web/API/Element/shadowRoot)

R8-03에서는 같은 페이지에서 팝업 명령 전 해당 후보 0개, 명령 후 열린 Shadow DOM의 추가 후보 1개, 반복 실행 후 추가 0개를 확인했다. 해당 영상 주소에 대한 HTTP 요청은 0건이었다. 닫힌 Shadow DOM·인증용 쿼리·blob 후보는 추가되지 않았다. 컨트롤러 테스트로 활성 문서에 대한 메시지, 팝업 발신 확인, 이동·감지 설정 변경 시 결과 폐기와 Native Host 미호출을 확인했다. 화면 클릭과 운영 사이트 인수는 미검증이다.
