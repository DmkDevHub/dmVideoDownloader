# ARCHITECTURE — 시스템 구조

## 1. 설계 결정

| 계층 | 기술 | 책임 |
|---|---|---|
| GUI | Tauri 2 + Vue 3 + TypeScript | 메인 화면, 설정 창, 상태 표시 |
| 앱 코어 | Rust/Tauri | 프로세스/작업 큐, 파일, 설정, IPC |
| Chrome 확장 | Manifest V3 + TypeScript | 감지·중복 제거·현재 탭 배지/팝업 |
| 브라우저 브리지 | Native Messaging Host (별도 Windows EXE) | stdio ↔ 로컬 IPC 중계 |
| 로컬 IPC | Windows Named Pipe 또는 대등한 로컬 전용 IPC | 호스트와 실행 중인 앱 간 전달 |
| 미디어 분석 | yt-dlp | 공개 URL 포맷 탐색, 다운로드 |
| 병합 | FFmpeg | 오디오·비디오 스트림 합성/컨테이너 처리 |
| 저장 | JSON | 설정 및 선택적 작업 상태; DB 없음 |

## 2. 프로세스 및 정보 흐름

```text
브라우저 웹페이지 (재생)
  │
  ├─ webRequest: URL/응답 MIME 등 허용된 메타데이터
  └─ content script: video/source 요소(접근 가능한 범위)
  ▼
Chrome MV3 확장 서비스 워커
  ├─ 후보 병합/분류/중복 제거
  ├─ 현재 탭 팝업/배지 (감지만 수행)
  └─ [다운로드 클릭]
  ▼
chrome.runtime.sendNativeMessage / connectNative
  ▼
VideoNativeHost.exe (stdio, 길이-프레이밍 JSON)
  ▼
로컬 전용 IPC (접근 제한/타임아웃)
  ▼
VideoDownloader.exe (Tauri GUI + Rust)
  ├─ yt-dlp 포맷 분석/실패 분류
  ├─ 사용자 설정을 반영한 다운로드 작업 큐
  ├─ yt-dlp 다운로드 / FFmpeg 병합
  ├─ settings.json / 선택적 downloads.json
  └─ 진행률·상태·Windows 알림
```

### 중요한 분리

- 확장은 **영상 바이트/쿠키/토큰/인증 헤더를 앱으로 보내지 않는다**.
- 감지 자체는 앱을 자동 실행하지 않는다. 팝업에서 사용자가 다운로드를 누른 경우에만 앱 실행/전달을 시도한다.
- Native Host는 GUI와 다른 프로세스이다. `stdout`은 프로토콜 전용이며 진단 텍스트는 `stderr` 또는 로컬 로그를 사용한다.
- 직접 URL 입력은 확장 연결이 없어도 동작한다.
- Chrome에서 로그인한 페이지도 감지될 수 있지만, **인증정보 재사용 기능은 없음**.

## 3. 권장 소스 폴더 구조 (구현 후)

```text
browser-video-downloader/
├─ AGENTS.md / CODEX_START.md / README.md / ROADMAP.md / CHANGELOG.md
├─ docs/
├─ apps/desktop/                   # Vue 3 + Tauri 2
│  ├─ src/                         # 화면/상태/타입
│  └─ src-tauri/                   # Rust 앱 코어
├─ extension/
│  ├─ manifest.json
│  └─ src/                         # service-worker/content/popup
├─ native-host/                    # Native Messaging 브리지
├─ shared/                         # 메시지 계약 또는 스키마 정의(필요 시)
├─ scripts/                        # Windows 등록/빌드·패키징 도구
└─ tests/                          # 테스트 자산/픽스처
```

위 디렉터리는 시작 제안이다. 이미 구현된 소스가 있다면 기존 구조를 최대한 존중한다. 런타임 산출물인 `dist/`는 빌드 시 생성한다.

## 4. 상태와 오류 모델

- **감지 상태:** `detected`, `analyzing`, `available`, `unsupported`, `requires_auth`, `possibly_protected`, `expired`.
- **작업 상태:** `queued`, `analyzing`, `downloading`, `merging`, `completed`, `paused`(가능한 경우), `failed`, `cancelled`.
- 감지된 모든 영상이 다운로드 가능한 것으로 보이면 안 된다. 분석 전에는 가능성 확정 금지.
- 로그는 `event kind`, 익명화한 후보 ID, 오류 코드 정도만 유지; 전체 URL 쿼리·헤더는 저장 금지.

## 5. 설계의 한계와 결정 보류

- `chrome.webRequest`는 관찰 가능한 요청만 보여 주며 모든 재생 미디어의 원본 URL을 보장하지 않는다.
- `blob:` 주소 자체를 다운로드 URL로 사용하지 않는다.
- HLS/DASH에서 최고 화질에 필요한 오디오/비디오가 다른 경로로 제공될 수 있다.
- **Chrome debugger 권한을 기본 MV3 확장에 넣지 않는다**. 릴리스 후 사례 기반으로 별도 검토.
- Tauri 포터블 패키징 시 WebView2 및 런타임 리소스 동봉/의존성은 Windows 실기에서 검증해야 한다.

## 6. 설계 검증 기준

- 모듈 경계와 I/O가 문서로 설명 가능할 것.
- 동일 영상이 후보/작업에서 안정적인 ID로 추적될 것.
- 앱이 꺼져 있는 상태와 Native Host가 미등록인 상태가 구분되어 진단될 것.
- 설정 변경은 새로운 다운로드 작업부터 스냅샷으로 적용될 것.

## 7. Chrome 설치·등록 책임 (설계 v1.2)

- 앱 설정 창: unpacked 설치 안내, 실제 ID 입력, 현재 경로 기반 Host 매니페스트 생성 및 사용자 HKCU 등록·복구·해제.
- Chrome 사용자: 개발자 모드 활성화, `extension/` 로드·새로고침·제거. EXE는 확장을 강제 설치하거나 기업 정책을 바꾸지 않는다.
- 확장 서비스 워커: `com.browser_video_downloader.host`로 연결을 시작하고 팝업 테스트 요청을 중계. EXE가 Native Messaging으로 확장을 먼저 호출하지 않는다.
- 런타임 파일 위치는 `PACKAGING.md`가 기준이다. 확장 폴더와 Host 경로 이동은 각각 재로드/ID 확인과 HKCU 재등록이 필요하다.
- 테스트 상태와 오류는 `SETTINGS.md`, 왕복 및 결과 확인 메시지는 `IPC_PROTOCOL.md`를 따른다. 설치 방식은 향후 스토어 배포와 분리하지만 v1.2 기본은 수동 unpacked 로드로 확정한다.
