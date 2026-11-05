# CHANGELOG

모든 개발 변경은 ROADMAP의 **세부 작업 1개 완료 단위**로 기록한다. 명세 작성과 실제 구현을 구분한다.

## 2026-11-05 — [R1-05] 설정 손상 복구와 프로세스 재시작 유지

- 변경: 손상 파일을 자동 덮어쓰지 않고 읽기전용 기본값/복구 안내 제공. 명시적 복구 시 고유 백업을 sync한 후 기본값 저장. 미래 스키마는 일반 저장·복구 모두 거부. 설정 재읽기와 안전한 오류 메시지 제공.
- 자동 테스트: `npm run verify` PASS(Vitest 10개, Rust 7개). 실제 숨김 앱 프로세스 4회 실행으로 저장/재시작/손상 복구/미래 버전 보존 검증. 외부 파일 검사로 손상 백업 바이트와 미래 파일 원본 보존 확인.
- 요구사항: TC-01/14 내부 통합 PASS. TC-15 설정 스냅샷 분리 부분 PASS, 진행 중 다운로드는 R5 미구현으로 NOT RUN. 화면 직접 조작 없음.
- 문서: ROADMAP R1-05 반영. 다음 승인 범위 R1-06 Chrome 설치·연결 UI 구성. 푸시 없음.

## 2026-11-05 — [R1-04] JSON 설정 저장·로드

- 변경: Rust settings.rs에 strict 스키마, Windows Known Folder 기본값, EXE 기준 data/settings.json 생성·로드, 임시 파일 sync/원자 교체/재읽기 확인 구현. 파일 공유 잠금·바이트 기준 낙관적 경합 검사로 중복 앱의 조용한 덮어쓰기 방지. 저장 실패 시 UI 유지.
- 자동 테스트: `npm run verify` PASS(Vitest 8개, Rust 5개). 타입/범위/알 수 없는 키/중복 키 및 ID 검사, legacy ID 누락 호환, 실제 교체 실패 시 원본 보존, 별도 Store 재로드 및 숨김 앱의 실제 저장 IPC 검증.
- 요구사항: TC-01 저장/재로드 부분 PASS. smoke는 별도 .test-output 경로를 사용하며 사용자 설정·다운로드 경로를 변경하지 않음. 프로세스 재시작·손상 복구는 R1-05. ROADMAP 반영, 푸시 없음.

## 2026-11-05 — [R1-03] Windows 폴더 선택 및 경로 검사

- 변경: Rust dialog 플러그인을 사용자 찾아보기 동작에 연결. folders.rs에서 Windows 로컬 절대경로·예약 이름·경로 순회·시스템/리파스 경로 검사 후 폴더 생성과 고유 임시 probe로 쓰기 확인. 기존 파일은 덮어쓰지 않음. JS 파일시스템 권한은 추가하지 않음.
- 자동 테스트: `npm run verify` PASS(Vitest 7개, Rust 2개, 숨김 WebView, 일반 빌드). TC-02 부분 검증: 위험 경로 거부, 한글 폴더·probe 정리·기존 파일 보존 및 picker 어댑터 취소/오류.
- 제한: 실제 Windows picker 화면 선택·ACL 거부는 미검증. 저장 연동은 다음 R1-04. ROADMAP 반영, 푸시 없음.

## 2026-11-05 — [R1-02] 단일 설정 대화상자와 편집 초안

- 변경: SettingsDialog.vue/settings.ts에 명세의 일반 옵션, 단일 모달, 초안/적용값 분리, 취소·기본값 복원 구성. Rust가 Windows 다운로드 위치를 조회해 기본 경로 결정. 저장/찾아보기는 후속 구현 전 비활성.
- 자동 테스트: `npm run verify` PASS(단위 6개, 실제 숨김 WebView에서 기본 경로 IPC 포함, 일반 디버그 빌드). 중복 열기·취소·기본값 미저장 동작 내부 모델 검사. 시각적 배치·화면 직접 조작 미검증.
- 문서: ROADMAP R1-02 완료 반영. TC-01/02/14의 저장·폴더 검증은 R1-03~05에서 수행. 다음 승인 범위 R1-03 순차 진행.

## 2026-11-05 — [R1-01] 메인 화면 배치

- 변경: App.vue/style.css에 URL 입력, 후보·다운로드 빈 목록, 연결 확인 필요 상태, 설정/폴더 버튼 배치. 미구현 동작은 비활성화하고 화질·진행률을 지어내지 않음.
- 자동 테스트: 타입 검사, Vitest 3개, 프런트엔드 빌드, 실제 숨김 Windows WebView smoke PASS. 화면 직접 조작 없음.
- 요구사항 검증: 컴포넌트 내부 렌더링과 실제 Vue 마운트 확인. 시각적 배치·제품 기능은 미검증. 직접 대응 TC 없음. ROADMAP 반영, 다음 승인 범위 R1-02 순차 진행.

## 2026-11-05 — [R0-04] 기본 내부 자동화 테스트 체계

- 변경: Vitest 실행 설정, Vue SFC 내부 렌더링 및 기본 네이티브 권한/CSP 단위 테스트 3개 추가. 루트 `npm test`, `npm run verify` 진입점과 개발 의존성 잠금 파일 갱신.
- 자동 테스트: `npm run verify` 전체 PASS (타입 검사, 2개 파일·3개 단위 테스트, 숨겨진 실제 Windows WebView/Vue/Rust 호출 및 정상 종료, 일반 디버그 빌드). Rust fmt 및 `git diff --check` PASS.
- 요구사항 검증: 화면 조작 없이 테스트 실행·실제 Windows 앱 구동을 확인하고 마지막 산출물을 일반 디버그 EXE로 복원. 직접 대응 TC 없음; TC-01~20 기능 테스트는 NOT RUN.
- 문서: README/BUILD/tests 안내, TEST_PLAN 상세 결과, ENVIRONMENT 후속 검증 및 ROADMAP R0-04/R0 전체 완료 반영.
- 남은 제한: 시각적 레이아웃·릴리스 배포·제품 기능 미검증. 다음 후보는 R1-01 하나이며 추가 사용자 지시 전 착수하지 않음. 푸시 없음.

## 2026-11-05 — [R0-03] 소스 폴더 경계 및 빌드 규칙

- 변경: `extension`, `native-host`, `shared`, `tests` 폴더의 책임/후속 구현 경계를 기록. 공통 TypeScript 설정을 `shared/tsconfig.base.json`으로 분리해 데스크톱에서 사용. `docs/BUILD.md`에 실제 빌드 명령·잠금 파일·산출물/제외 규칙 정리.
- 자동 테스트: 폴더/필수 파일·공통 설정 연결 PowerShell 검사, `npm run build:debug`, `git diff --check` PASS. npm/Cargo 잠금 파일 변경 없음.
- 요구사항 검증: 모듈 경계와 기존 Windows 앱 빌드 확인. 확장 manifest/실행은 R2, Native Host 프로토콜/EXE는 R3, 공통 메시지 타입/스키마는 R3-01에서 구현하며 현재 기능이 있다고 표시하지 않음. 직접 대응 TC 없음, TC-01~20 NOT RUN.
- 문서: README 및 ROADMAP R0-03 완료 반영. 다음은 승인된 R0-04 테스트 체계 구성.

## 2026-11-05 — [R0-02] 최소 Windows 앱 실행 골격

- 변경: `apps/desktop`에 Tauri 2 + Vue 3 + TypeScript + Rust 골격 구성. 루트 npm 워크스페이스, npm/Cargo 잠금 파일, 최소 CSP·권한 없는 설정 및 빌드 산출물 제외 규칙 추가. 영상 감지·다운로드·설정 기능은 미구현.
- 자동 테스트: `npm run check`, `npm run build:debug`, `npm run smoke`, `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`, `git diff --check` PASS.
- 요구사항 검증: Windows 11 Pro에서 숨겨진 실제 네이티브 WebView 창 생성→Vue 마운트→Rust 호출→정상 종료 확인. 화면 클릭·입력 없음. 테스트 IPC는 별도 feature/프런트엔드 모드로 한정. 직접 대응 TC 없음, TC-01~20 NOT RUN.
- 문서: README 실행 명령, ENVIRONMENT 환경 재확인 결과 및 ROADMAP R0-02 완료 반영. CIM 읽기 조회로 기존 Windows 제품명 미확정 해소.
- 남은 제한: 시각적 레이아웃·사용자 조작·릴리스 배포 미검증. 다음은 사용자 승인 범위의 R0-03을 순차 진행.

## 2026-11-05 — 새 컴퓨터의 R0 재확인 규칙 반영

- R0 완료 표시와 관계없이 새 컴퓨터에서 최초 개발 전 환경·기존 빌드·내부 자동화 테스트를 재확인하도록 AGENTS, CODEX_START, README에 명시.
- ENVIRONMENT에 환경별 결과 추가 방식 명시. 기존 소스·조사 이력·완료 체크는 보존하고 실패·미검증은 별도 기록. 후속 개발 자동 착수 없음.
- 검증: `git diff --check` 및 문서 간 정책 일치 확인. 문서 변경으로 앱 테스트는 해당 없음. 현재 컴퓨터의 R0-01 조사 이력은 이미 존재하며 다른 컴퓨터의 검증은 수행하지 않음.

## 2026-11-05 — 내부 자동화 검증 규칙 반영

- 사용자 지시에 따라 AGENTS, CODEX_START, TEST_PLAN에 화면 직접 조작 없이 내부 자동화 테스트로 검증하는 규칙 반영. 브라우저/데스크톱 클릭·입력 자동화도 제외.
- 실제 사용 인수 조건은 유지하며 내부 자동화로 확인하지 못한 조건은 미검증/보류로 기록하도록 명시.
- 검증: `git diff --check` 및 변경 문서의 정책 일치 확인. 문서 변경으로 앱 테스트는 해당 없음. ROADMAP 체크 상태 변경 및 다음 개발 작업 착수 없음.

## 2026-11-05 — 작업 완료 후 로컬 커밋 규칙 변경

- 사용자 지시에 따라 AGENTS, README, CODEX_START의 Git 규칙을 완료·검증한 작은 작업마다 로컬 커밋 생성, 푸시 금지로 통일.
- 직전 완료된 R0-01 조사 기록과 함께 로컬 커밋으로 정리. 다음 ROADMAP 항목은 착수하지 않음.
- 검증: 문서 간 Git 규칙 일치 및 `git diff --check` 확인. 문서 변경이므로 앱 테스트는 해당 없음.

## 2026-11-05 — [R0-01] 기존 프로젝트·환경·파일 조사

- 변경: `docs/ENVIRONMENT.md`에 기존 설계 문서 15개 보존 목록, 소스/설정 부재, Windows·Node·Rust·Chrome·빌드 도구 조사 결과 기록. ROADMAP의 R0-01만 완료 처리.
- 자동 검증: PowerShell 파일 수·확장자·소스/런타임 경로 부재 검사 PASS. Node/npm/Rust/Cargo 버전 및 Rust 타깃 실행 확인. 변경 대상 외 기존 문서 13개의 SHA-256 보존 비교 수행.
- 요구사항 검증: 환경·파일 조사 완료. 직접 대응 TC ID 없음. TC-01~20 NOT RUN(앱·확장·Native Host 미구현).
- 남은 제한: Windows 제품명 확정 보류(CIM 접근 불가), Tauri CLI 미설치, 실제 컴파일·창 실행 미검증. R0 단계 전체 미완료. 다음 후보는 R0-02이며 미착수.
- Git 명령, 도구 설치, 브라우저/레지스트리 변경은 수행하지 않음.

## 2026-11-05 — 설계 문서 v1.2

- 기존 v1.1의 15개 Markdown을 유지하며 Chrome 설치·연결 설계를 구체화.
- EXE 설정 창 설치 안내, Chrome 개발자 모드 unpacked 로드, 실제 확장 ID 입력 및 단일 allowlist 확정.
- EXE의 현재 사용자 HKCU 등록, 고정 Host 이름/절대경로, 연결 테스트·복구·해제 및 폴더 이동/ID 변경 재등록 규칙 추가.
- 확장에서 시작하는 PING/PONG 및 결과 확인 계약, 오류·타임아웃·상태 표시 정의. 웹스토어 강제 설치/기업 정책 변경 제외.
- PRD/아키텍처/UI/보안/개발 지시와 ROADMAP 및 TC-16~20을 정합화. 기존 구현 체크는 모두 미완료 유지.
- 설계 버전은 v1.2. IPC 버전과 설정 스키마는 1 유지, `browserExtensionId`는 누락 시 빈 값으로 호환.
- 검증 범위: 문서 링크·요구사항 반영·버전/계약 정합성 및 전체 ZIP 내용 일치 검사. 앱 구현 및 Windows/Chrome 실행 테스트는 NOT RUN.

## 2026-11-05 — 설계 문서 v1.1

- 단일 대형 명세 대신 `AGENTS.md`, `ROADMAP.md` 및 역할별 `docs/*.md`로 분리.
- Chrome 자동 감지, Windows 포터블 다운로드, JSON 설정, 비인증 콘텐츠 범위를 확정.
- **코드 구현·실행 검증은 아직 수행하지 않음.**

## 이후 구현 기록 템플릿

### YYYY-MM-DD — [R?-??] 작업 제목

- 변경: 파일/기능
- 자동 테스트: 수행 항목, 성공/실패
- 요구사항 검증: TC ID 및 실제 동작
- 남은 제한: 필요 시 기록
