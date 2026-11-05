# 공통 타입 및 설정 경계

`tsconfig.base.json`은 데스크톱과 향후 확장 코드에 사용할 공통 TypeScript 검사 설정이다.

Rust ↔ TypeScript 메시지 필드와 버전의 정본은 `../docs/IPC_PROTOCOL.md`이다. 실제 공통 메시지 타입·스키마는 R3-01에서 이 폴더에 추가한다. 현재 메시지 계약 구현은 없으며 임시 인증 필드나 임의의 메시지 형식을 정의하지 않는다.

공통 코드는 Vue UI, Chrome API, Tauri 런타임에 의존하지 않도록 유지한다. 앱과 Native Host는 별도 프로세스로 빌드한다.
