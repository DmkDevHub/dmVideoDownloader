# BROWSER_EXTENSION — Chrome MV3 영상 자동 감지 설계

## 1. 목표와 범위

Chrome 확장 프로그램이 페이지에서 재생 중인 **비인증 미디어 후보**를 관찰하고, 탭별 목록/아이콘 배지/팝업에 노출한다. 사용자가 팝업에서 [다운로드]를 클릭하면 후보 메타데이터만 Native Messaging Host로 보낸다. 실제 다운로드는 확장이 수행하지 않는다.

## 2. 구성 요소

| 구성 | 역할 |
|---|---|
| `manifest.json` | MV3, 필요한 권한, 서비스 워커, content script, action 정의 |
| Background Service Worker | 요청/응답 메타데이터 관찰, 후보 캐시, 탭 관리, Native Messaging 호출 |
| Content Script | 접근 가능한 HTML5 `video`, `source` 요소를 관찰해 미디어 후보 보완 |
| Popup UI | 현재 탭의 후보 목록, 분석 상태, 다운로드 클릭 및 연결 오류 표시 |
| Session Store | 서비스 워커 재시작 시 탭별 후보를 복구하기 위한 `chrome.storage.session` 등 |

### 권한 정책

- `webRequest`는 **관찰 용도**로 사용한다. 요청 수정/차단은 하지 않는다.
- `nativeMessaging`은 확장-호스트 통신용.
- `storage`, 필요한 탭 정보와 호스트 접근 권한만 사용한다.
- 여러 사이트 자동 감지를 위한 호스트 접근은 사용자에게 분명히 설명하고, Chrome의 사이트 접근 제어를 존중한다.
- `cookies`, `debugger`는 기본 확장에 포함하지 않는다. Cookie API, 로그인 세션 및 비밀키 추출 기능은 구현 금지.
- `webRequest`에서 요청/응답 헤더 전체를 수집하지 않는다. 가능하면 **URL, 메서드, 유형, 명시적 콘텐츠 유형 정도의 허용 목록 메타데이터**만 사용한다. `Cookie`, `Authorization`, `Set-Cookie` 등의 값은 어떤 경로로도 전달/보관하지 않는다.

## 3. 감지 소스와 처리 순서

1. `webRequest`에서 직접 동영상 파일(`.mp4`, `.webm`), 재생목록(`.m3u8`, `.mpd`) 관련 요청을 분류한다.
2. 응답에서 관찰 가능한 MIME(`video/*`, `application/vnd.apple.mpegurl`, `application/dash+xml`, 기타 미디어 타입)을 참고한다.
3. content script가 HTML5 `video.currentSrc` 및 `source.src` 등을 읽을 수 있는 범위 내에서 보조 후보를 보낸다.
4. 후보를 `tabId + page identity + media identity` 기준으로 그룹화한다.
5. `.ts`, `.m4s`, 분할 `.mp4` 등 개별 세그먼트는 독립 영상으로 나열하지 않는다.
6. 비디오/광고/추적 관련 오탐 가능성을 분류하되 근거 없이 특정 광고를 완벽하게 배제한다고 보장하지 않는다.
7. 사용자가 탭을 닫거나 다른 페이지로 이동하면 관련 후보를 정리한다. **현재 탭 기준** 배지·목록을 표시한다.
8. 후보의 다운로드 가능 여부/해상도는 아직 분석 전이므로 `확인 필요`로 표시한다.

## 4. 후보 데이터 모델 (초안)

```ts
export type SourceType = 'mp4' | 'webm' | 'hls' | 'dash' | 'unknown';

export interface VideoCandidate {
  candidateId: string;
  tabId: number;
  pageUrl: string;        // 일시적 전달용. 영구 저장/원문 로깅 금지
  pageTitle?: string;
  mediaUrl?: string;       // blob:은 네트워크 URL로 사용하지 않음
  sourceType: SourceType;
  detectedAt: string;     // ISO 8601
  status: 'detected' | 'analyzing' | 'available' | 'unsupported';
}
```

원시 HTTP 인증 헤더, 브라우저 쿠키, 사용자 정보, 사이트 세션, 비밀번호를 모델에 넣지 않는다. URL 자체가 일회성 접근 권한을 포함할 수 있으므로 메시지/로그 처리 전에 위험한 쿼리 매개변수를 판별하며, 명백한 인증정보 내장 URL은 대상에서 제외한다.

## 5. UI/동작

- 확장 아이콘 배지는 **현재 탭의 후보 개수**를 표시한다.
- 팝업: 제목(가능할 경우), 사이트, 감지 유형, 분석 상태, [다운로드] 버튼.
- [다운로드] 클릭 시에만 Native Host로 보내고, 연결 실패 시 사용자가 이해할 수 있는 원인과 [연결 설정 안내]를 제시한다.
- 자동 감지 사용 여부는 앱 설정과 동기화하지만, 앱이 미실행일 때는 마지막에 확인된 설정을 확장 로컬 저장소에 반영하는 등 **동기화 실패 정책**을 구현 전에 정한다. 감지 결과가 앱을 자동 실행시키지 않는다.
- 서로 다른 탭의 목록이 섞이지 않도록 한다.

## 6. 경계 사례

- `blob:` URL: 브라우저 내부 참조일 수 있다. 원본 미디어 요청이 없으면 `직접 다운로드 주소 확인 불가`.
- `data:` URL: 디폴트 다운로드 대상으로 처리하지 않음.
- DRM/EME: 보호 여부를 확실히 판정하지 못하면 `보호 여부 미확인`으로 표시. 키/복호화 정보 추출 금지.
- 인증 필요 콘텐츠: 재생 요청이 관찰되더라도 자격 증명을 재사용하지 않음.
- WebSocket, MSE 등 복잡한 플레이어: 기본 자동 감지 범위 밖일 수 있음.
- URL/미디어 메타데이터는 페이지 출처가 제공하는 신뢰하지 않는 입력으로 취급.

## 7. 개발 단계 및 테스트

- **R2-01~R2-06**에 나누어 구현한다. Native Host와 직접 결합하기 전에 감지 후보의 타입·필터링 테스트를 만든다.
- 테스트: TC-03(직접 영상), TC-04(HLS 감지), TC-05(DASH 감지), TC-06(세그먼트 중복), TC-09(연결 오류).
- 영상 테스트용으로 외부 유료/인증 사이트를 사용하지 않는다. 공개 테스트 자산 또는 로컬 개발용 HTTP 테스트 서버의 비인증 영상만 사용한다.

공식 참조: https://developer.chrome.com/docs/extensions/reference/api/webRequest

## 8. 설치 및 연결 절차 (v1.2)

기본 배포는 **포터블 EXE 설정 창의 안내 + unpacked 확장 수동 로드**다. 별도 설치 창을 만들지 않고 설정 창 안에서 순서와 현재 상태를 보여준다. [확장 프로그램 설치 안내]는 설치를 완료하는 버튼이 아니다.

1. ZIP 전체를 보관할 쓰기 가능한 폴더에 풀고 `VideoDownloader.exe`를 실행한다. 임시 압축 해제 경로에서 확장을 로드하지 않는다.
2. [설정] → [Chrome 확장 프로그램] → [확장 프로그램 설치 안내]를 누른다. EXE는 **빌드된** `extension/` 폴더를 탐색기로 열고 Chrome의 `chrome://extensions` 열기를 시도한다.
3. 내부 주소 열기가 실패하면 [Chrome 확장 관리 주소 복사]로 주소를 복사해 Chrome 주소창에 직접 붙여넣는다. Chrome이 없거나 조직 정책으로 차단되면 그 사유를 안내한다.
4. 사용자가 Chrome에서 개발자 모드를 켜고 [압축해제된 확장 프로그램을 로드합니다]를 눌러 `manifest.json`이 있는 `extension/`을 선택한다. ZIP 파일, 전체 포터블 폴더, TypeScript 소스 폴더를 선택하지 않는다.
5. Chrome 확장 카드의 ID를 복사해 EXE 설정 창의 [확장 ID]에 입력한다. 팝업에도 `chrome.runtime.id` 값을 읽기 전용으로 표시하고 복사 버튼을 제공한다. EXE가 Chrome 프로필 파일을 읽어 ID나 설치 여부를 추측하지 않는다.
6. [호스트 등록]을 누르면 EXE가 ID를 검증·저장하고 매니페스트 및 HKCU 등록을 갱신한다. ID가 없거나 잘못됐으면 등록하지 않는다. 첫 등록의 상세 계약은 `PACKAGING.md`를 따른다.
7. EXE [연결 테스트]를 누른 뒤 안내에 따라 **확장 팝업의 [연결 테스트]**를 누른다. 확장→Host→실행 중 EXE→확장 왕복을 확인한다. 성공 응답이 돌아오기 전에는 설치·연결 완료로 표시하지 않는다.

설치는 Chrome 사용자/프로필별로 수행한다. 다른 프로필에서는 같은 확장을 별도로 로드해야 한다. Chrome 사이트 접근 권한이 제한되어 감지가 안 되는 상태는 Native Messaging 연결 실패와 구분한다. 개발자 모드 사용에 따른 Chrome 안내를 숨기거나 보안 설정을 우회하지 않는다.

## 9. 확장 ID allowlist와 유지보수

- 기본 연결 대상은 사용자가 확인한 ID **한 개**다. 형식은 소문자 `a`~`p` 32자이며, 형식 검사만으로 공식 확장임이 증명되지는 않는다. 사용자는 이 배포 폴더에서 로드한 확장인지 확인한다.
- Host 매니페스트의 `allowed_origins`는 해당 ID의 `chrome-extension://<ID>/`만 포함한다. 와일드카드·모든 확장 허용·연결 요청에서 받은 ID의 자동 추가를 금지한다.
- ID 변경 시 새 ID 입력 후 [호스트 등록] 또는 [연결 복구/재등록]으로 **기존 ID를 대체**하고 다시 테스트한다. 거부된 확장이 자기 자신을 allowlist에 추가할 수 없다.
- unpacked ID는 고정이라고 가정하지 않는다. 확장 폴더 이동·재로드 시 Chrome에 표시된 실제 ID를 다시 확인한다. v1.2는 수동 ID 확인을 기준으로 하며 고정 키 배포를 전제로 하지 않는다.
- 폴더를 이동했다면 새 경로의 EXE로 호스트를 재등록한다. Chrome이 옛 확장 경로를 참조하면 기존 항목을 제거하고 새 `extension/`을 로드한다. 제거 전 필요한 상태를 확인하고 사용자 조작으로 진행한다.
- 확장 파일 업데이트 후 `chrome://extensions`에서 새로고침하고 필요한 페이지를 다시 연다. 웹스토어 자동 업데이트는 제공하지 않는다.
- EXE는 확장을 몰래 설치·활성화하지 않는다. 웹스토어 강제 설치, 외부 확장 레지스트리 설치, `ExtensionInstallForcelist` 같은 기업 정책 변경을 하지 않는다. 향후 웹스토어 배포는 별도 작업이며 IPC 계약과 분리한다.

## 10. 설치 관련 검증과 근거

R1-06, R2-07, R3-04~09, R7-02~05 / TC-09/12/16~20을 따른다. 이 문서는 구현 요구사항이며 실제 Chrome 설치 테스트를 수행했다는 뜻이 아니다.

- [Chrome 공식 unpacked 로드 안내](https://developer.chrome.com/docs/extensions/get-started/tutorial/hello-world#load-unpacked)
- [Chrome 공식 Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)

공식 규격 확인일: 2026-11-05. 설치 버튼·ID 수동 확인·복구 UI는 본 제품의 설계 결정이다.

## 11. R2 구현 결정 (2026-11-06)

- 개발 unpacked 로드는 `npm run build:extension`의 **`extension/dist/`**를 선택한다. 포터블 배포 시에는 이 내용 전체를 배포 폴더 `extension/`에 복사한다(R7). TypeScript 소스 폴더 자체를 로드하지 않는다.
- `webRequest.onResponseStarted`는 관찰 전용이며 추가 옵션은 `responseHeaders`만 사용한다. Content-Type만 즉시 투영하고 헤더 원본은 보관하지 않는다. `extraHeaders`·요청 헤더·Cookie API는 사용하지 않는다.
- `webNavigation`은 root/iframe 문서 ID와 이동 정리를 위해 사용한다. `tabs`/`debugger`/`downloads` 권한은 없다. HTTP(S) 사이트 접근은 자동 감지에 필요하며 Chrome에서 사이트별 제한 가능하다.
- 후보는 trusted-context `storage.session`에만 저장한다. 루트/프레임 문서 ID와 현재 URL을 대조해 복구하고 이동·종료 때 지운다. 최대 20탭/탭당 30개/전체 200개. 알려진 세그먼트는 보수적으로 제외하며 불명확한 분할 MP4/광고의 완벽한 판별을 보장하지 않는다.
- 설정 동기화 정책: 감지는 기본 켜짐이며 앱의 마지막 확인값이 있으면 비밀값 없는 자동 감지 옵션만 `storage.local`에 유지한다. 앱 연결이 없으면 마지막 확인값/기본값을 사용하고 팝업에 동기화 대기임을 표시한다. 감지를 위해 Native Host나 앱을 자동 실행하지 않는다. 실제 앱 설정 교환과 왕복 검증은 R3이며 R2의 로컬 상태 검증으로 대체하지 않는다.
- 확장은 후보 URL을 재요청하지 않는다. URL의 위험 스킴·인식 가능한 인증정보·내부 IP를 거부하지만 DNS/리다이렉트/다운로드 하위 요청 검증은 엔진 R4에서 별도 수행해야 한다.
- 같은 탭에서 출처·경로 범위·미디어 식별 쿼리가 일치하는 알려진 HLS/DASH 변형은 하나의 후보로 묶는다. 마스터를 나중에 발견하면 기존 후보 ID를 유지하며 대표 주소를 바꾼다. 서로 다른 콘텐츠 ID와 독립 파일은 유지한다. 이름으로 식별할 수 있는 조각·별도 음성 요청은 후보에서 제외한다. 판단이 모호한 주소는 독립 후보로 유지한다.

## 11. R8 선택 탭 상세 감지 범위

열린 Shadow DOM에서 미리 로드하지 않는 공개 영상을 놓치는 사례에 한해 적용한다. 팝업의 [현재 탭 상세 감지]를 누르면 최상위 문서에 이미 실행 중인 콘텐츠 스크립트가 열린 Shadow DOM을 한 번 확인한다. 자동 감지가 켜져 있고 Chrome이 사이트 접근을 허용한 탭에서 사용할 수 있다. 한 번에 요소 5,000개, 반환 후보 30개까지 확인하며 기존 공개 주소 검증과 후보 중복 제거를 사용한다.

이 범위는 기존 콘텐츠 스크립트 메시징으로 처리할 수 있다. 디버거·스크립트 삽입 권한과 상시 상세 감지는 재현된 문제 해결에 필요하지 않아 보류한다. 닫힌 Shadow DOM, 브라우저 내부 주소, 인증·보호 콘텐츠에서 원본 주소나 데이터를 얻는 기능은 제품 범위에서 제외한다. [Chrome 탭 메시징](https://developer.chrome.com/docs/extensions/reference/api/tabs#method-sendMessage)
- DOM 출발 메시지 전에도 URL을 검증하고 MIME은 판별된 정식 유형만 전달한다. 자동 감지 off는 content script에도 전달하여 DOM 스캔/메시지를 중단한다. 사이트 제공 텍스트는 HTML로 해석하지 않으며 제목에 포함된 URL도 호스트명으로 정제한다.
- 확장 진단은 동일 포트/요청 ID의 PING→PONG→CONNECTION_TEST_RESULT→ACK와 총 30초 만료를 검증한다. 성공 포트는 종료하고 세션에는 마지막 성공 시각만 남긴다. 실제 Host/EXE 왕복·설정 교환은 R3 검증이며 내부 포트 테스트로 성공을 대체하지 않는다. ID 복사는 사용자가 버튼을 누를 때만 클립보드 쓰기를 시도한다.
