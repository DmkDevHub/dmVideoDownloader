# 공통 프로토콜 및 검사 설정

`tsconfig.base.json`은 데스크톱과 확장의 TypeScript 검사 설정이다. `protocol.ts`와 `protocol/`은 v1 메시지를 검증하고 `protocol-fixtures.json`의 같은 입력을 테스트한다.

Rust 크레이트는 Host와 앱이 공유한다. Windows 프레이밍, 매니페스트 및 Named Pipe 구현이 포함된다. TypeScript 검증기는 Vue와 Chrome 런타임에 의존하지 않는다. [IPC 계약](../docs/IPC_PROTOCOL.md)
