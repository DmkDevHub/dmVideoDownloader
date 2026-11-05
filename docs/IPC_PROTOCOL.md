# IPC_PROTOCOL — 확장 ↔ Native Host ↔ Tauri 메시지 계약

## 1. 원칙

- Chrome MV3 확장은 Chrome **Native Messaging**으로 별도 `VideoNativeHost.exe`와 통신한다.
- Host는 stdio에서 메시지를 받은 뒤 로컬 IPC(예: Windows Named Pipe)로 Tauri 데스크톱 앱에 중계한다.
- **미디어 바이너리/사용자 쿠키/인증 헤더/로그인 세션은 절대 전달하지 않는다.**
- 사용자가 팝업의 [다운로드]를 선택한 후에만 앱으로 후보를 전달하고, 평소 감지 이벤트는 확장 내부에 유지한다.

## 2. Chrome Native Messaging 규격

- 메시지: `uint32` 길이 프리픽스(네이티브 바이트 순서) + UTF-8 JSON 본문. 프로세스 종료/오버사이즈/부분 읽기를 처리한다.
- 메시지 크기는 공식 브라우저 제한을 준수하고 프로젝트 내부 한도는 더 보수적으로 정한다. 큰 바이너리/목록을 한 번에 넣지 않는다.
- Native Host의 **stdout은 프로토콜 전용**, 진단은 stderr/로그에 쓴다.
- 앱 실행 여부, IPC 연결 제한 시간, 오류 유형을 명확하게 구분한다.
- Host 매니페스트 `allowed_origins`는 **정확한 확장 ID**만 허용, 와일드카드 금지.
- 레지스트리의 Native Host 경로는 사용자별 HKCU 등록으로 관리하고, 포터블 경로 변경 시 재설정한다.

## 3. 프로토콜 버전 1

기본 엔벌로프:

```json
{
  "version": 1,
  "type": "VIDEO_CANDIDATE",
  "requestId": "req-0001",
  "payload": {
    "candidateId": "candidate-0001",
    "tabId": 17,
    "pageUrl": "https://example.org/watch/123",
    "pageTitle": "Public sample",
    "mediaUrl": "https://media.example.org/master.m3u8",
    "sourceType": "hls",
    "detectedAt": "2026-11-05T00:00:00Z"
  }
}
```

- `pageUrl`과 `mediaUrl`은 공개 URL만 허용하며 둘 중 한 주소를 이용해 분석할 수 있도록 한다. 로그인 자격정보가 포함된 URL은 지원하지 않는다.
- `sourceType`: `mp4`, `webm`, `hls`, `dash`, `unknown`.
- `requestId`는 응답을 원 요청에 연결하고, `candidateId`는 확장 후보의 논리적 ID다.
- `detectedAt`: ISO 8601. `tabId`는 브라우저 내부 숫자로 외부 계정과 무관.
- 외부 입력은 길이, 타입, URL 스킴/호스트, 중복 필드·잘못된 JSON 여부를 검사한다.

권장 메시지 타입:

| 타입 | 방향 | 목적 |
|---|---|---|
| `PING` / `PONG` | 양방향 | 연결 진단 및 버전 확인 |
| `VIDEO_CANDIDATE` | 확장 → Host → 앱 | 사용자가 선택한 영상 후보 전달 |
| `GET_RUNTIME_STATUS` / `RUNTIME_STATUS` | 확장 ↔ 앱 | 앱의 연결·감지 설정 동기화 |
| `ACK` | 앱 → 확장 | 명시적인 수신·검증 결과 |
| `ERROR` | 앱/Host → 확장 | 식별 가능한 오류 코드와 안전한 설명 |

응답 예시:

```json
{
  "version": 1,
  "type": "ACK",
  "requestId": "req-0001",
  "payload": { "accepted": true, "candidateId": "candidate-0001" }
}
```

## 4. 로컬 IPC 제약

- Windows 로컬 전용 Named Pipe 등으로 구현하고 외부 네트워크 포트를 열지 않는다.
- 사용자별 접근 제어(ACL)와 수신자 인증/연결 주체 검증을 실제 Windows 환경에서 확인한다.
- Host가 보내는 데이터라도 신뢰하지 않는다. 앱 코어에서 재검증한다.
- 로컬 앱이 꺼져 있을 때 앱을 실행하는 것은 **사용자의 다운로드 클릭에 따른 전송 과정에서만** 허용한다. 실행 실패 시 적절한 메시지 제공.
- 같은 요청의 중복 수신은 `requestId` 기준으로 중복 작업이 생성되지 않도록 제어한다.
- 메시지 전달 성공은 다운로드 성공을 의미하지 않는다.

## 5. 오류 코드 초안

`HOST_NOT_REGISTERED`, `EXTENSION_ID_MISMATCH`, `HOST_START_FAILED`, `APP_NOT_RUNNING`, `APP_LAUNCH_FAILED`, `IPC_TIMEOUT`, `INVALID_PAYLOAD`, `UNSUPPORTED_VERSION`, `URL_NOT_ALLOWED`, `DOWNLOAD_ENGINE_UNAVAILABLE`.

모든 오류에 `code`, `safeMessage`를 제공하되, 비밀정보/URL 전체를 출력하지 않는다.

## 6. 테스트 기준

- 크기·형식 오류, 메시지 분할/연속 수신, 잘못된 버전, 중복 요청, 정해진 권한 외 연결 시도.
- Chrome `allowed_origins`의 정확성 및 Host 등록·해제, 앱 종료/실행, 포터블 이동.
- TC-09, TC-12 및 R3-01~R3-06을 기준으로 완료 여부 판단.

공식 참조: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging

## 7. 설치·진단 연결 계약 (v1.2 문서 개정, 프로토콜 v1 유지)

- 고정 Native Host 이름: `com.browser_video_downloader.host`. HKCU 및 매니페스트 계약은 `PACKAGING.md` 3절을 따른다.
- ID 등록은 EXE 설정 창의 사용자 동작으로만 수행한다. `PING`이나 다른 메시지의 ID 필드로 allowlist를 수정하지 않는다. Host는 Chrome이 전달하는 호출 origin도 예상 `chrome-extension://<ID>/`와 대조한다. 브라우저 확장 ID는 사이트 계정·인증 세션과 무관하다.
- EXE [연결 테스트]는 경로/등록 사전 검사 후 30초간 대기한다. Native Messaging 호출은 확장 서비스 워커에서 시작하며 content script가 직접 호출하지 않는다.
- 연결 테스트의 PING/PONG·결과 확인/ACK는 하나의 `connectNative` 포트에서 수행한다. 단발 `sendNativeMessage` 호출을 섞어 연결 동일성 검증을 깨뜨리지 않는다. 테스트 종료 시 포트를 닫고, 재검사 전에는 마지막 성공 시각만 남긴다.
- 확장 팝업 [연결 테스트]는 `version: 1`, `type: "PING"`, 매번 새 `requestId`, `payload: {"purpose":"connection_test"}`를 전송한다. Host는 실행 중 EXE로 중계하고 EXE는 동일 ID의 `PONG`, `payload: {"protocolVersion":1,"appReachable":true}`를 응답한다.
- 확장은 유효한 `PONG` 수신 후 동일 요청 ID로 `CONNECTION_TEST_RESULT`, `payload: {"success":true}`를 전송한다. EXE는 대기 중 실제로 발급한 PONG의 ID·연결·제한 시간을 확인하고 성공 상태를 기록한다. EXE가 이 결과에 `ACK`를 반환하면 팝업도 성공을 표시한다. 다른 메시지에는 기존 중복 요청 방지 규칙을 적용하되 이 테스트 후속 메시지는 타입과 단계까지 구분한다.
- 최초 대기 시간이 지나거나 임의 ID/다른 연결의 결과가 오면 성공 처리하지 않는다. Host 자체 PONG만으로 앱까지 연결됐다고 표시하지 않는다. 진단 메시지는 앱이나 다운로드를 자동 실행하지 않는다.
- 포트 종료·테스트 시간 초과·경로/ID 변경 시 현재 연결 성공을 무효화한다. 앱에 표시하는 마지막 성공 시각은 이력 정보이며 항상 현재 연결을 보장하지 않는다.
- 확장 미설치/비활성 상태는 EXE에서 직접 확정할 수 없을 수 있다. Chrome 오류가 모호하면 `연결 확인 실패`와 확인 순서를 제공한다. `EXTENSION_ID_MISMATCH`는 로컬 설정/매니페스트 비교 등 근거가 있을 때만 확정한다.

추가 오류: `HOST_PATH_MISMATCH`, `REGISTRATION_FAILED`, `CONNECTION_TEST_TIMEOUT`. 구조화된 프로젝트 오류를 만들 수 없는 Chrome 호출 실패는 `HOST_START_FAILED`와 정제된 설명으로 처리하고 원인 단정을 피한다. 메시지 버전은 설치 방식 변경만으로 올리지 않는다.
