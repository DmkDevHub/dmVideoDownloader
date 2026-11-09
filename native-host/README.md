# Windows Native Messaging Host

루트에서 `npm run build:host`를 실행한다. Windows 산출물은 `shared/protocol/target/debug/VideoNativeHost.exe`이다. 개발 연결용 앱·Host·확장 폴더는 `npm run stage:connection`으로 만든다.

Host는 Chrome이 전달한 origin을 자기 폴더의 단일 allowlist와 대조한다. 길이 접두어가 있는 v1 JSON을 stdin/stdout으로 교환하고, 같은 폴더의 실행 중인 Tauri 앱에 Windows Named Pipe로 중계한다. 앱이 꺼져 있으면 실행 안내를 반환한다. 등록과 해제는 앱 설정에서 한다. [IPC 계약](../docs/IPC_PROTOCOL.md)

`npm run test:native`는 분할 프레임과 실제 Windows 프로세스의 연속 프레임·오류 처리를 확인한다. Chrome과 앱의 통합 검증 절차는 BUILD에 있다. [빌드와 검증](../docs/BUILD.md)
