# PACKAGING — Windows 포터블 배포 및 Chrome 연결 (v1.2)

## 1. 포터블 배포 원칙

- 목표는 ZIP 전체를 압축 해제하여 실행하는 Windows 폴더 배포다. 모든 요소를 하나의 EXE로 합치지 않는다.
- Chrome 확장은 EXE 설정 창에서 안내하고 사용자가 개발자 모드로 unpacked 로드한다. 웹스토어 등록이나 강제 설치, 기업 정책 변경은 기본 배포에 없다.
- EXE가 사용자의 [호스트 등록] 또는 [연결 복구/재등록] 동작으로 **현재 사용자 HKCU**만 변경한다. 관리자 권한이나 HKLM 등록을 요구하지 않는다.
- 포터블은 레지스트리를 전혀 쓰지 않는다는 뜻이 아니다. Native Messaging을 사용하지 않는 직접 URL 입력은 호스트 등록 없이 동작한다.
- 실제 Tauri 2 실행 파일·리소스·WebView2 의존성과 바이너리 재배포 조건은 Windows 11에서 검증한다. 기본 설치 패키지를 포터블 결과물과 혼동하지 않는다.

## 2. 목표 런타임 결과물 구조

```text
VideoDownloaderPortable/
├─ VideoDownloader.exe
├─ VideoNativeHost.exe
├─ tools/
│  ├─ yt-dlp.exe
│  └─ ffmpeg.exe
├─ extension/                     # Chrome에 로드할 빌드 결과 전체
│  ├─ manifest.json
│  ├─ background.js
│  ├─ content.js
│  ├─ popup.html
│  └─ popup.js                     # 필요 리소스도 모두 동봉
├─ native-host-manifest.json       # EXE가 현재 경로와 ID로 생성
├─ data/
│  └─ settings.json
├─ logs/                          # 필요할 때만 생성
└─ README.txt                     # 설치/연결/이동/해제 절차
```

이 구조는 향후 앱 빌드 산출물이다. **설계 ZIP v1.2에는 Markdown 15개만 들어 있다.** 소스의 `native-host/`는 개발 폴더이며 배포 EXE의 위치는 위 구조를 기준으로 한다. 실행 파일명은 모든 등록 코드와 일치해야 한다. `downloads.json`은 이력·복구가 필요할 때만 추가한다.

## 3. 최초 설치와 등록

1. 쓰기 가능한 최종 보관 위치에 전체 폴더를 풀고 `VideoDownloader.exe`를 실행한다. 기본 설정 위치는 `data/settings.json`이다.
2. 설정 창 [Chrome 확장 프로그램]에서 [확장 프로그램 설치 안내]로 폴더와 Chrome 확장 관리 페이지를 연다. 내부 주소 열기 실패 시 주소 복사 안내를 제공한다.
3. 사용자가 `chrome://extensions` → 개발자 모드 → [압축해제된 확장 프로그램을 로드합니다]에서 `extension/`을 선택한다. 세부 절차는 `BROWSER_EXTENSION.md` 8절을 따른다.
4. Chrome에 표시된 확장 ID를 설정 창에 입력하고 [호스트 등록]을 누른다. EXE는 ID, 현재 실행 파일 기준 경로, 호스트 파일 존재 및 쓰기 권한을 검증한다. 현재 작업 디렉터리를 기준으로 경로를 계산하지 않는다.
5. EXE는 매니페스트를 임시 파일에 작성·검증한 뒤 교체하고 아래 HKCU 키 기본값을 매니페스트 **절대경로**로 기록한다. 설정 ID 저장 실패 시 등록하지 않는다. 레지스트리 쓰기 실패·부분 실패는 표시하고 테스트 성공 상태를 지운다. 재시도 가능한 상태를 유지한다.
6. 등록 후 읽어온 값과 현재 경로를 대조한다. 등록 성공만으로 `연결됨`으로 표시하지 않고 [연결 테스트]를 안내한다.

### 고정 호스트 이름과 경로 계약

- 호스트 이름: `com.browser_video_downloader.host`
- 레지스트리: `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.browser_video_downloader.host`
- 기본값 타입: `REG_SZ`; 값은 JSON 파일의 절대경로(명령줄 인수가 아님).
- 매니페스트 `path`: 현재 폴더의 `VideoNativeHost.exe` 절대경로.
- 확장 `connectNative`/`sendNativeMessage`는 위 호스트 이름을 사용한다.
- Windows 레지스트리 32/64비트 뷰를 진단한다. 다른 뷰의 오래된 동일 호스트 등록이 우선 적용되어 실패하는 경우를 테스트하고, 이 앱 소유임이 확인되는 항목만 정리한다.

아래 JSON은 경로·ID 형식 예시다. ID는 실제 사용자의 값으로 생성하며 이 예시를 그대로 배포하지 않는다.

```json
{
  "name": "com.browser_video_downloader.host",
  "description": "Browser Video Downloader bridge",
  "path": "D:\\VideoDownloaderPortable\\VideoNativeHost.exe",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://abcdefghijklmnopabcdefghijklmnop/"]
}
```

확장 ID 형식은 `[a-p]{32}`로 검증하며 `allowed_origins`에는 정확한 ID 한 개만 둔다. 새 ID 등록은 기존 ID를 대체한다. HTTP 수신 서버나 자동 allowlist 확대를 추가하지 않는다.

## 4. 연결 테스트와 복구

Chrome Native Messaging은 **확장이 연결을 시작**한다. EXE가 등록만 하고 확장 설치 완료를 단정하거나 Chrome 없이 자체 PING하여 전체 연결을 입증해서는 안 된다.

- EXE [연결 테스트]: 로컬 등록/경로 사전 검사 후 30초간 테스트 대기 상태로 전환하고 확장 팝업을 열도록 안내한다.
- 확장 [연결 테스트]: `PING`을 Host와 실행 중 EXE에 전달한다. EXE 응답이 확장까지 돌아오면 확장이 동일 요청 ID의 `CONNECTION_TEST_RESULT`를 보내 성공 확인을 완료한다. 상세 메시지는 `IPC_PROTOCOL.md`를 따른다.
- 앱 미실행이면 진단 시 자동 실행하지 않고 `APP_NOT_RUNNING`을 안내한다. 호스트 자체 응답만으로 전체 연결 성공 처리하지 않는다.
- 연결 성공 시 마지막 성공 시각, 확인한 ID/경로/프로토콜 버전을 표시한다. 테스트 종료 후에는 마지막 확인 결과로 표시하며, 연결 종료·시간 초과·경로/ID 변경 시 이전 성공을 현재 연결로 재사용하지 않는다.
- [연결 복구/재등록]: 현재 ID와 현재 EXE 경로로 매니페스트 및 HKCU 값을 다시 생성하고 읽어 검증한다. 재연결 테스트가 필요함을 표시한다. 확장 누락·비활성화·사이트 권한·정책 차단은 사용자가 Chrome에서 조치하도록 안내한다.
- 같은 작업을 여러 번 눌러도 중복 등록이나 허용 ID 누적이 없어야 한다. 기존 등록이 다른 위치를 가리키면 기존/새 경로를 보여주고 사용자의 복구 버튼 동작으로 대체한다. 다른 복사본도 동시에 연결된다고 약속하지 않는다.

## 5. 폴더 이동, 업데이트, 연결 해제

1. 앱과 진행 중 작업을 종료한 뒤 폴더 전체를 옮긴다. 새 위치 EXE에서 등록 값·매니페스트·호스트 실제 경로의 불일치를 감지한다.
2. 설정 [연결 복구/재등록]으로 새 절대경로를 등록한다. 옛 폴더가 없어도 새 위치에서 복구할 수 있어야 한다.
3. 확장 폴더도 옮겼다면 Chrome에서 옛 항목을 확인·제거하고 새 `extension/`을 로드한다. 표시된 실제 ID가 바뀌면 입력값을 갱신하고 다시 등록한다.
4. 확장 새로고침 및 필요한 탭 재로드 후 연결 테스트를 다시 수행한다. 경로 복구만으로 확장 재설치까지 완료했다고 표시하지 않는다.

업데이트는 `data/settings.json`을 보존한다. 확장 빌드 파일이 바뀌면 Chrome에서 새로고침한다. 첫 버전에는 자동 업데이터가 없다.

[연결 해제]는 현재 복사본의 매니페스트를 가리키는 이 앱의 HKCU 키만 제거한다. 다른 복사본 경로를 가리키면 소유권 불일치를 안내하고 임의 삭제하지 않는다. 확장 삭제는 사용자가 Chrome에서 수행한다. 폴더 삭제만으로 레지스트리가 지워지지 않으므로 삭제 전 연결 해제를 안내한다. 해제는 설정·다운로드 파일·다른 앱 등록을 삭제하지 않는다.

## 6. 배포 검증과 근거

- 일반 사용자 Windows 11에서 전체 압축 해제, 최초 설치, 설정 저장, 연결 테스트·복구·해제: TC-09/16~20.
- 공백/한글 포함 새 경로 이동, 옛 폴더 제거, ID 변경 후 새 allowlist, 다른 복사본의 등록 보호: TC-12/18~20.
- EXE/확장 빌드 리소스/yt-dlp/FFmpeg 누락과 WebView2 의존성, 코드 서명·보안 경고, 재배포 조건: TC-13 및 릴리스 기록.
- 수동 unpacked 설치, HKCU 경로와 정확한 `allowed_origins`는 [Chrome 설치 안내](https://developer.chrome.com/docs/extensions/get-started/tutorial/hello-world#load-unpacked), [Native Messaging 공식 규격](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)을 확인했다(2026-11-05). [Tauri 2 문서](https://v2.tauri.app/)는 실제 패키징 시 재확인한다.

R3에서 Chrome for Testing의 임시 프로필로 실제 Host 등록·후보 전달·왕복 테스트와 앱 폴더 이동 후 복구를 검증했다. 최종 ZIP, 일반 Chrome 수동 설치 및 일반 사용자 계정 인수는 R7 범위다. [R3 검증 기록](TEST_PLAN.md#10-r3-브라우저와-windows-연결-검증-2026-11-09)
