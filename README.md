# Browser Video Downloader

- 설계 버전: **v1.2** (2026-11-05)
- 대상: Windows 11 개인용 브라우저 감지 기반 영상 다운로더
- 개발 지향: **Chrome 영상 감지 → 사용자의 다운로드 선택 → Windows 포터블 앱에서 최고 화질 저장**
- 상태: **R0 완료 / R1 메인 화면·설정 저장·Chrome 설치 안내 구현**. 영상 감지·다운로드·Native Messaging 연결 기능은 아직 구현되지 않았다.

## 한눈에 보는 요구사항

- Chrome 확장(Manifest V3)으로 브라우저의 **비인증 영상**을 자동 감지한다.
- MP4/WebM 및 공개 HLS(m3u8)/DASH(mpd) 스트림을 분석한다.
- 감지 목록 및 직접 URL 입력을 제공하고 **사용자 클릭 시에만** 다운로드한다.
- 가능한 범위에서 최고 화질(분리 영상·음성 병합 포함)을 저장한다.
- Windows **포터블 폴더 형태**로 제공하며 Tauri 2 + Vue 3 + Rust + yt-dlp + FFmpeg를 사용한다.
- 메인 화면 1개와 **설정 창 1개**를 두고 `settings.json`에 저장한다.
- **계정/로그인, 쿠키·토큰·인증 세션 활용, DRM 우회, DB, 클라우드 기능은 제외**한다.

## Codex 시작 순서

1. 이 폴더 전체를 프로젝트 루트에 복사한다.
2. **`AGENTS.md`부터 읽고**, `docs/PRD.md`, `docs/ARCHITECTURE.md`, `ROADMAP.md`를 확인한다.
3. `CODEX_START.md`의 첫 지시문을 Codex에 전달한다.
4. **`ROADMAP.md`에서 가장 작은 미완료 작업 하나만** 구현하고 테스트·요구사항 검증·CHANGELOG를 함께 갱신한다.
5. 완료·검증한 작은 작업마다 로컬 커밋을 생성하며 푸시는 하지 않는다. 다음 항목으로의 진행은 사용자 요청을 기다린다.

다른 컴퓨터에서 클론하여 개발을 시작할 때는 R0 완료 여부와 관계없이 해당 컴퓨터에서 한 번 환경과 기존 빌드·내부 자동화 테스트를 재확인한다. 현재 환경의 확인 기록이 없거나 환경 변경 여부가 불명확한 경우도 확인한다. 화면은 직접 조작하지 않으며 결과는 `docs/ENVIRONMENT.md`에 환경별로 추가하고 기존 소스·기록·완료 체크는 보존한다.

## 문서 역할과 읽기 순서

| 파일 | 책임 |
|---|---|
| `AGENTS.md` | Codex 상시 개발 규칙(최상위 지침) |
| `CODEX_START.md` | 첫 실행 시 전달할 지시문 |
| `ROADMAP.md` | 유일한 작업 순서/상태 관리 문서; 작은 체크박스 단위 |
| `CHANGELOG.md` | 실제로 완료한 작업 단위의 변경 기록 |
| `docs/PRD.md` | 제품 목표, 기능 요구사항, 제외 범위 및 인수 기준 |
| `docs/ARCHITECTURE.md` | 실행 프로세스, 전체 흐름, 폴더 구조, 기술 선택 |
| `docs/BROWSER_EXTENSION.md` | unpacked 설치 안내, MV3 자동 감지, 확장 ID, 팝업·배지 |
| `docs/IPC_PROTOCOL.md` | Chrome ↔ Native Host ↔ Tauri 메시지 계약 |
| `docs/DOWNLOAD_ENGINE.md` | yt-dlp, FFmpeg, 화질 선택, 작업 상태 |
| `docs/SETTINGS.md` | 설정 UI, Chrome 설치·연결 테스트·복구, JSON 저장 |
| `docs/UI_SPEC.md` | 화면 요소, 상태, 오류 표현 |
| `docs/SECURITY_SCOPE.md` | 인증·DRM 제외, 입력 검증 및 개인정보 보호 |
| `docs/TEST_PLAN.md` | 자동 테스트와 실제 브라우저 검증 시나리오 |
| `docs/PACKAGING.md` | 포터블 배포 및 Chrome 연결·해제 절차 |
| `docs/BUILD.md` | 소스 폴더 경계, 현재 빌드 범위, 산출물 및 잠금 파일 |
| `docs/ENVIRONMENT.md` | 컴퓨터별 개발 환경과 검증 이력 |

## 요구사항 충돌 시 우선순위

`AGENTS.md`의 작업 규칙 → `docs/SECURITY_SCOPE.md`의 명시적 제외 → `docs/PRD.md`의 제품 요구사항 → 각 상세 설계 → `ROADMAP.md`의 실행 순서. 불일치가 발견되면 임의로 가정하지 말고 문서에 남기고 사용자에게 결정을 요청한다.

## 구현상 주의

- **감지 성공은 다운로드 성공이 아니다.** 로그인 필요, 만료 URL, DRM 및 브라우저 내부 주소는 실패할 수 있다.
- 초기 단계에서 사이트별 전용 추출기, 브라우저 디버거 권한, 영상 재인코딩 등을 무작정 확대하지 않는다.
- 공식 규격은 구현 시점에 확인한다: [Chrome Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging), [Chrome webRequest](https://developer.chrome.com/docs/extensions/reference/api/webRequest), [Tauri 2](https://v2.tauri.app/), [yt-dlp](https://github.com/yt-dlp/yt-dlp), [FFmpeg](https://ffmpeg.org/documentation.html).

## v1.2 Chrome 설치 방식

제품 구현 시 포터블 EXE의 설정 창에서 [확장 프로그램 설치 안내]를 연다. 사용자는 `chrome://extensions`에서 개발자 모드를 켜고 배포 폴더의 `extension/`을 [압축해제된 확장 프로그램을 로드합니다]로 선택한다. 확장 ID를 설정 창에 입력한 뒤 EXE가 현재 사용자 HKCU에 Native Messaging 호스트를 등록한다. 확장 팝업에서 시작하는 왕복 연결 테스트를 통과해야 `연결됨`으로 표시한다.

폴더를 이동하면 새 EXE에서 [연결 복구/재등록]을 실행하고, 확장 폴더도 이동했다면 새 위치를 다시 로드하고 ID를 확인한다. Chrome 웹스토어 등록·강제 설치·기업 정책 변경은 기본 배포에 포함하지 않는다. 상세 절차는 [설치 안내](docs/BROWSER_EXTENSION.md#8-설치-및-연결-절차-v12)와 [포터블 배포](docs/PACKAGING.md)를 따른다.

최초 설계 ZIP은 15개 Markdown 문서로 구성되었다. 현재 저장소에는 단계별로 앱 소스를 추가한다. 다운로드 도구는 아직 동봉하지 않는다. 설계 버전 v1.2와 IPC `version: 1`, 설정 `schemaVersion: 1`은 서로 다른 버전이다.

## 로컬 개발 및 내부 검증

Windows x64에서 Node.js 22.12 이상, Rust MSVC 타깃, Visual Studio C++ Build Tools/Windows SDK 및 WebView2가 필요하다. 처음 클론한 컴퓨터에서는 [환경 기록](docs/ENVIRONMENT.md)과 실제 도구 상태를 다시 확인한다. 프로젝트 설치는 루트에서 `npm ci`를 실행한다. npm 및 Cargo 잠금 파일을 함께 보존한다.

| 명령 (프로젝트 루트) | 동작 |
|---|---|
| `npm run check` | Vue/TypeScript 타입 검사 |
| `npm test` | 화면 조작 없는 Vitest 단위 테스트 (렌더링·설정 초안/취소·ID·연결 상태·권한/CSP) |
| `npm run test:rust` | 폴더·설정 파일·원자 교체·복구·경합·리소스 진단 테스트. 처음에는 프런트엔드 빌드 필요 |
| `npm run verify` | 타입 검사→Vitest→숨김 네이티브 smoke→Rust 테스트→일반 디버그 빌드. 실패 시 중단 |
| `npm run build:frontend` | 타입 검사 및 Vue 번들 생성 |
| `npm run build:debug` | 프런트엔드 및 Windows 디버그 EXE 빌드, 설치 패키지 없음 |
| `npm run build` | 릴리스 EXE 빌드, 설치 패키지 없음 (배포 검증은 R7) |
| `npm run smoke` | 격리 폴더에서 숨김 앱 4회 실행: 설정 저장/재시작/손상 복구/미래 버전 보존, 단일 설정창 모델 검증. 화면 클릭·입력 없음 |
| `npm run dev` | 사용자가 앱을 개발 모드로 실행할 때 사용 |

일반 실행 파일은 `apps/desktop/src-tauri/target/debug/dm-video-desktop.exe`이며 프런트엔드 산출물은 `apps/desktop/dist/`이다. smoke 명령은 같은 디버그 경로에 테스트 전용 실행 파일을 생성하므로 일반 실행 전 `npm run build:debug`로 다시 빌드한다. 테스트 IPC는 `smoke-test` Cargo feature와 smoke 프런트엔드 모드에만 포함된다. 숨겨진 창 검증은 시각적 레이아웃 확인을 대신하지 않는다.

구현 시 참고한 공식 문서: [Tauri Windows 선행 조건](https://v2.tauri.app/start/prerequisites/), [Vite 프런트엔드 연결](https://v2.tauri.app/start/frontend/), [설정](https://v2.tauri.app/reference/config/), [테스트](https://v2.tauri.app/develop/tests/).

단위 테스트는 [Vitest](https://vitest.dev/config/)와 [Vue 내부 문자열 렌더러](https://vuejs.org/api/ssr)를 사용한다. 렌더러는 테스트 프로세스 안에서만 사용하며 서버를 실행하지 않는다.

## 현재 설정 기능과 제한

메인 화면의 [설정]은 하나의 모달 대화상자를 연다. 설정은 실행 파일 옆 `data/settings.json`에 보관하며 기본 저장 위치는 Windows 다운로드 Known Folder 아래 `Videos`이다. [기본값 복원]은 편집값만 바꾸고 [저장] 전에는 적용하지 않는다. 저장 실패 시 창을 유지하고 이유를 안내한다.

저장 위치는 로컬 드라이브의 안전한 절대경로만 허용한다. 상대·UNC·장치·경로 순회·예약 이름·리파스 지점·보호 시스템 폴더는 거부한다. 존재하지 않는 저장 폴더는 선택/저장 과정에서 생성할 수 있으며 임시 파일로 쓰기를 검사한다. 설정 변경은 새 작업부터 적용하는 구조이며 실제 다운로드 작업은 아직 없다.

손상 설정은 자동으로 덮어쓰지 않는다. 같은 설정창의 [손상 파일 백업 후 기본값 저장]을 선택하면 `data/settings.backup-*.json`에 원본을 보관한 뒤 복구한다. 미래 스키마는 저장·복구를 거부하며 호환 앱이 필요하다. 다른 앱이 설정을 변경했다면 덮어쓰지 않고 재시작을 안내한다. 앱 폴더에 쓸 수 없으면 쓰기 가능한 위치로 포터블 폴더 전체를 옮겨 실행한다.

Chrome 영역은 수동 unpacked 설치 안내, 관리 주소 복사, 실제 확장 ID 입력/설정 저장 및 로컬 배포 리소스 진단을 제공한다. 경로는 실행 파일 기준이다. **호스트 등록·연결 테스트·복구·해제 버튼은 R3 구현 전까지 비활성화**되어 있다. 파일 존재나 ID 형식만으로 설치됨/연결됨을 표시하지 않는다. 현재 저장소의 extension 폴더는 아직 Chrome에 로드 가능한 빌드 결과가 아니다.

자동 검증은 `.test-output/smoke-*`의 격리 데이터만 사용한다. 사용자 설정·다운로드 폴더와 레지스트리를 변경하지 않고 Chrome/탐색기/폴더 선택창을 실행하지 않는다. 시각적 배치와 실제 OS 선택창 조작은 미검증이며 상세 범위는 [테스트 기록](docs/TEST_PLAN.md)을 따른다.
