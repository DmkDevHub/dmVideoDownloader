# R0-01 — 기존 프로젝트 및 개발 환경 조사

조사일: 2026-11-05 (Asia/Seoul). 대상: `D:\workspace\dm_workspace\dmVideoDownloader`.
이 기록은 환경 조사 결과이며 앱 빌드·실행 또는 R0 단계 전체의 완료를 뜻하지 않는다.

## 기존 상태와 보존 대상

`rg --files --hidden -g '!.git/**'` 및 `Get-ChildItem -Force`로 확인한 기존 파일은 Markdown 15개이다.

- 루트: `AGENTS.md`, `README.md`, `CODEX_START.md`, `ROADMAP.md`, `CHANGELOG.md`.
- docs: `PRD.md`, `ARCHITECTURE.md`, `SECURITY_SCOPE.md`, `BROWSER_EXTENSION.md`, `IPC_PROTOCOL.md`, `DOWNLOAD_ENGINE.md`, `SETTINGS.md`, `UI_SPEC.md`, `TEST_PLAN.md`, `PACKAGING.md`.
- 기존 `.git/` 디렉터리는 보존한다. 이번 작업에서 Git 명령은 실행하지 않았다.
- 앱·확장·Native Host 소스, `package.json`, `Cargo.toml`, `node_modules`, `dist`, `settings.json`, `downloads.json`은 조사 경로에 없다. 기존 앱 설정이나 사용자 다운로드 파일은 발견되지 않았다.
- 변경은 조사 기록 신규 작성과 ROADMAP/CHANGELOG 갱신으로 한정한다. 나머지 기존 문서 13개는 SHA-256 비교로 보존 여부를 검증한다.

## 실제 확인한 환경

| 항목 | 결과 및 확인 방법 |
|---|---|
| Windows | 레지스트리 `HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion`: Build `26300`, UBR `9711`, DisplayVersion `26H2`, ProductName `Windows 10 Pro`. AMD64. 제품명과 빌드 표기가 혼재하므로 Windows 11 여부 확정 보류. `Get-CimInstance Win32_OperatingSystem`은 CIM 접근 불가로 실패 |
| PowerShell | `7.6.5`. 명령 시작 시 FileSystem provider의 InitializeDefaultDrives 경고가 출력되었으나 프로젝트 파일 조회 및 도구 실행은 가능 |
| Node.js | `node --version` → `v24.12.0` |
| npm | `npm --version` → `11.6.2` |
| Rust | `rustc --version` → `1.99.1 (58cf8f9c3 2026-10-13)` |
| Cargo | `cargo --version` → `1.99.1 (5f94df478 2026-08-27)` |
| Rust 툴체인/타깃 | `rustup show active-toolchain` → `stable-x86_64-pc-windows-msvc (default)`; `rustup target list --installed` → `x86_64-pc-windows-msvc` |
| Tauri | `Get-Command tauri` 미발견; `cargo tauri --version` → `no such command: tauri` (종료 코드 1). 프로젝트 의존성도 없어 Tauri 2 설치·실행 미검증 |
| Chrome | `C:\Program Files\Google\Chrome\Application\chrome.exe` 파일 존재, ProductVersion `156.0.8078.80`. 실행·확장 로드는 하지 않음 |
| Visual Studio Build Tools | `vswhere.exe -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json` → Build Tools 2022 `17.14.37806.7`, 설치 완료, 재부팅 불필요. MSVC 디렉터리 `14.44.35207` 존재 |
| MSVC PATH | 현재 셸의 `Get-Command cl`, `Get-Command link` 미발견. 위 설치 확인과 별개로 실제 Rust 링크·컴파일은 미검증 |
| Windows SDK | `C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0` 존재. 실제 빌드 미검증 |
| WebView2 | EdgeUpdate Clients 레지스트리에서 `Microsoft Edge WebView2 런타임`, `156.0.4314.58` 확인. 앱에서 로딩은 미검증 |
| yt-dlp / FFmpeg | `Get-Command`로 PATH에서 미발견. 프로젝트에 동봉 바이너리 없음. 시스템 전체 미설치를 단정하지 않음 |

도구 설치·업데이트, 레지스트리 쓰기, Chrome 프로필 조회, 네트워크 요청은 수행하지 않았다.

## 검증과 완료 판단

- 자동 조사 검증 PASS: PowerShell에서 기존 루트/docs 파일 수가 15개이고 모두 `.md`인지 검사하고, 위 소스·설정·산출물 경로가 없는지 `Test-Path`로 검사했다. 불일치 시 `throw`하도록 실행했으며 종료 코드 0이었다.
- 실행 확인 PASS: Node/npm/Rust/Cargo 버전과 Rust 툴체인·타깃을 실제 명령으로 확인했다. Tauri CLI 미설치는 실패 결과 그대로 기록했다.
- 문서 보존 검증: 수정 대상 2개를 제외한 기존 13개 문서의 작업 전후 SHA-256을 비교한다.
- `docs/TEST_PLAN.md`의 TC-01~20: 모두 NOT RUN. 앱·확장·Native Host 소스가 없으므로 설정, 감지, 다운로드, 연결, 포터블 이동 시나리오를 실행할 수 없다. R0-01 환경 조사에 직접 대응하는 TC ID는 없으며 새 ID를 임의로 추가하지 않는다.
- R0-01의 완료 기준인 기존 상태·환경·보존 목록 기록은 충족한다. Windows 11 제품명 확정, Tauri 개발 의존성 준비, 실제 컴파일·창 실행은 남아 있다. R0 단계 전체는 미완료이다.

다음 후보는 R0-02(최소 Tauri 2 + Vue 3 + TypeScript + Rust 실행 골격) 한 개이며, 사용자 지시 전에는 착수하지 않는다.
