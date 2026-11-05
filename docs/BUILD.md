# 소스 경계와 빌드 규칙

## 폴더별 책임 및 현재 구현 상태

| 경로 | 책임 | 현재 빌드 여부 |
|---|---|---|
| `apps/desktop/src` | Vue 3 화면 | Vite로 빌드 가능, 실행 골격만 구현 |
| `apps/desktop/src-tauri` | Tauri 2/Rust 앱 코어 | Windows EXE 빌드 가능, 다운로드·설정·IPC 기능 미구현 |
| `extension` | Chrome MV3 감지·팝업 | 경계 안내만 존재, manifest 및 빌드는 R2-01 |
| `native-host` | 별도 Windows Native Messaging EXE | 경계 안내만 존재, 프로토콜/빌드는 R3 |
| `shared` | 런타임에 의존하지 않는 공통 타입·검사 설정 | 공통 TS 설정을 데스크톱이 사용, 메시지 타입/스키마는 R3-01 |
| `scripts` | 빌드·내부 검증 진입점 | 숨김 WebView 실행 검증 제공 |
| `tests` | 공통 자동화 검증·자산 | 단위 테스트 실행 체계는 R0-04 |

확장·호스트 경계 폴더가 존재하는 것은 기능 실행 가능을 뜻하지 않는다. 다운로드는 앱이 맡고 Chrome은 감지와 명시적 선택 전달만 맡는다. 설치·연결 관련 구현 전에 해당 설계 문서를 읽는다.

## 재현 가능한 빌드

1. 새 컴퓨터에서는 AGENTS 규칙에 따라 R0 환경을 한 번 확인하고 ENVIRONMENT에 결과를 추가한다.
2. 루트에서 `npm ci`로 `package-lock.json` 기준 의존성을 복원한다. 전역 Tauri CLI는 필요 없다.
3. `npm run check` 및 `npm run build:frontend`로 Vue/TypeScript를 확인한다.
4. `npm run build:debug`로 앱 EXE를 만든다. 릴리스 컴파일은 `npm run build`이며 R7 배포 인수 검증을 대신하지 않는다.
5. `npm run smoke`로 화면 조작 없이 숨겨진 실제 WebView·Vue·Rust 호출을 검증한다. 정상 실행용 EXE가 필요하면 이후 `npm run build:debug`를 실행한다.

Rust 잠금 파일은 `apps/desktop/src-tauri/Cargo.lock` 하나이며 해당 Cargo manifest가 앱의 빌드 경계이다. 직접 확인할 때는 루트에서 `cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml`을 사용한다. 프런트엔드가 먼저 빌드되어 있어야 한다. 소스 변경이 없을 때 잠금 파일 변경은 의도하지 않는다. Native Host 구현 전에는 빈 Cargo 워크스페이스나 호스트 EXE를 만들지 않는다.

| 산출물 | 경로 | 저장소 포함 |
|---|---|---|
| 프런트엔드 | `apps/desktop/dist` | 제외 |
| 디버그 EXE | `apps/desktop/src-tauri/target/debug/dm-video-desktop.exe` | 제외 |
| 릴리스 EXE | `apps/desktop/src-tauri/target/release/dm-video-desktop.exe` | 제외 |
| npm/Cargo 잠금 파일 | 루트 `package-lock.json`, 앱 `Cargo.lock` | 포함 |

`.gitignore`는 `node_modules`, Rust `target`, 프런트엔드 `dist`, Tauri 생성 스키마와 검증 산출물을 제외한다. 파일 삭제·초기화로 빌드 문제를 해결하지 말고 먼저 원인을 확인한다. 배포 폴더·도구 동봉·포터블 이동은 `PACKAGING.md`와 R7에서 다룬다.
