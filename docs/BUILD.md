# 소스 경계와 빌드 규칙

## 폴더별 책임 및 현재 구현 상태

| 경로 | 책임 | 현재 빌드 여부 |
|---|---|---|
| `apps/desktop/src` | Vue 3 화면 | 메인 화면·단일 설정 대화상자·Chrome 안내/상태 UI |
| `apps/desktop/src-tauri` | Tauri 2/Rust 앱 코어 | Windows EXE, 설정, HKCU 등록, Named Pipe 수신과 연결 진단 |
| `extension` | Chrome MV3 감지·팝업 | `npm run build:extension` → `extension/dist`, Chrome unpacked 로드 대상 |
| `native-host` | 별도 Windows Native Messaging EXE | `npm run build:host`로 `shared/protocol/target/debug/VideoNativeHost.exe` 생성 |
| `shared` | 공통 프로토콜과 검사 설정 | Rust·TypeScript v1 검증, 공통 테스트 입력, Windows 프레임·파이프 |
| `scripts` | 빌드·내부 검증 진입점 | 숨김 WebView 실행 검증 제공 |
| `tests` | 공통 자동화 검증·자산 안내 | 단위 테스트는 앱의 `src/**/*.test.ts`, `npm test`로 실행 |

확장·호스트 경계 폴더가 존재하는 것은 기능 실행 가능을 뜻하지 않는다. 다운로드는 앱이 맡고 Chrome은 감지와 명시적 선택 전달만 맡는다. 설치·연결 관련 구현 전에 해당 설계 문서를 읽는다.

## 재현 가능한 빌드

1. 새 컴퓨터에서는 AGENTS 규칙에 따라 R0 환경을 한 번 확인하고 ENVIRONMENT에 결과를 추가한다.
2. 루트에서 `npm ci`로 `package-lock.json` 기준 의존성을 복원한다. 전역 Tauri CLI는 필요 없다.
3. `npm run check` 및 `npm run build:frontend`로 Vue/TypeScript를 확인한다.
4. `npm run build:debug`로 앱 EXE를 만든다. 릴리스 컴파일은 `npm run build`이며 R7 배포 인수 검증을 대신하지 않는다.
5. `npm run smoke`로 화면 조작 없이 숨겨진 실제 WebView·Vue·Rust 호출을 검증한다. 정상 실행용 EXE가 필요하면 이후 `npm run build:debug`를 실행한다.

`npm run verify`는 타입 검사, Vitest, 숨김 데스크톱 스모크, Rust 단위 테스트, Host 프레임·실제 Named Pipe 전달 검증, 일반 디버그 EXE와 확장 빌드를 순서대로 실행한다. 개별 `npm run test:bridge`는 `npm run smoke`로 만든 숨김 앱 바이너리와 `npm run build:host` 산출물을 사용한다. 검증 뒤 `npm run build:debug`로 일반 바이너리를 만든다.

`npm run test:extension:chrome`은 Chrome for Testing의 임시 headless 프로필과 로컬 HTTP fixture에서 실행한다. `DM_CHROME_BINARY`로 브라우저 실행 파일을 지정할 수 있다. `DM_MEDIA_FIXTURE`에 공개 테스트 MP4 절대경로를 지정하면 재생 시간 증가도 검사한다. R3 통합은 `DM_NATIVE_APP`과 `DM_NATIVE_HOST`를 모두 지정해 실행한다.

```powershell
npm run smoke
npm run build:host
npm run build:extension
$env:DM_NATIVE_APP = (Resolve-Path apps/desktop/src-tauri/target/debug/dm-video-desktop.exe).Path
$env:DM_NATIVE_HOST = (Resolve-Path shared/protocol/target/debug/VideoNativeHost.exe).Path
npm run test:extension:chrome
npm run build:debug
```

R3 모드는 동일 호스트 이름의 기존 등록 값을 발견하면 중단한다. 임시 앱이 운영 등록·복구·해제 명령을 실행한다. 테스트 중 생성된 키는 테스트 경로를 다시 대조해 정리한다. 팝업과 앱에 내장된 메시지·DOM 상태를 검사하며 수동 설치와 시각 인수는 별도로 기록한다.

smoke 테스트 전용 feature에서만 `DM_SMOKE_ROOT`/`DM_SMOKE_PHASE`를 사용한다. 테스트는 별도 `.test-output/smoke-*`에서 네이티브 프로세스를 순차 실행하고 설정 파일과 백업을 검사한다. 일반 앱은 환경 변수로 설정 경로를 변경하지 않으며 EXE 폴더 기준으로만 `data/settings.json`을 사용한다. 테스트 산출물은 커밋하지 않는다.

Rust 잠금 파일은 데스크톱, `native-host`, `shared/protocol`에 각각 있다. 각 Cargo 명령은 해당 manifest와 잠금 파일을 사용한다. 개발 연결을 확인할 때는 `npm run stage:connection`으로 `dist/connection-*`에 새 실행 폴더를 만든다. 릴리스 동봉과 배포 검증은 R7에서 진행한다.

| 산출물 | 경로 | 저장소 포함 |
|---|---|---|
| 프런트엔드 | `apps/desktop/dist` | 제외 |
| 디버그 EXE | `apps/desktop/src-tauri/target/debug/dm-video-desktop.exe` | 제외 |
| 릴리스 EXE | `apps/desktop/src-tauri/target/release/dm-video-desktop.exe` | 제외 |
| npm/Cargo 잠금 파일 | 루트 `package-lock.json`, 데스크톱·Host·프로토콜 `Cargo.lock` | 포함 |

`.gitignore`는 `node_modules`, Rust `target`, 프런트엔드 `dist`, Tauri 생성 스키마와 검증 산출물을 제외한다. 파일 삭제·초기화로 빌드 문제를 해결하지 말고 먼저 원인을 확인한다. 배포 폴더·도구 동봉·포터블 이동은 `PACKAGING.md`와 R7에서 다룬다.
