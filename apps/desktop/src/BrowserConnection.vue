<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { connectionLabel, registrationLabel, validExtensionId, type BrowserRuntime, type ConnectionStatus } from './browserConnection';

const props = withDefaults(defineProps<{
  extensionId: string; savedId: string; busy: boolean; canSave: boolean;
  runtime?: BrowserRuntime; status?: ConnectionStatus;
}>(), { status: () => ({ phase: 'needsCheck', roundTripConfirmed: false, lastSuccessAt: null }) });
defineEmits<{ 'update:extensionId': [id: string]; registration: [action: 'register' | 'repair' | 'unregister' | 'test'] }>();
const expanded = ref(false);
const message = ref('');
const actionsUnavailable = computed(() => props.busy || !props.canSave || !props.runtime?.connectionActionsAvailable || props.status.phase === 'waiting');
async function openFolder() {
  try { await invoke('open_extension_folder'); message.value = '확장 폴더 열기를 요청했습니다. manifest.json이 있는 폴더를 선택하세요.'; }
  catch { message.value = '빌드된 확장 폴더를 열 수 없습니다. 배포 폴더의 extension/과 manifest.json을 확인하세요.'; }
}
async function showGuide() {
  expanded.value = !expanded.value;
  if (!expanded.value) return;
  const results = await Promise.allSettled([invoke('open_extension_folder'), invoke('open_chrome_extensions')]);
  message.value = results.some(result => result.status === 'rejected')
    ? '폴더 또는 Chrome을 열지 못했습니다. 아래 경로와 관리 주소를 이용하세요.'
    : '폴더와 Chrome 열기를 요청했습니다. 관리 화면이 열리지 않으면 아래 주소를 복사하세요.';
}
async function copyAddress() {
  try { await navigator.clipboard.writeText('chrome://extensions'); message.value = 'Chrome 확장 관리 주소를 복사했습니다.'; }
  catch { message.value = '주소를 복사하지 못했습니다. 아래 관리 주소를 직접 선택해 복사하세요.'; expanded.value = true; }
}
</script>

<template>
  <fieldset :disabled="busy">
    <legend>Chrome 확장 프로그램</legend>
    <p>확장 설치: 설치 확인 필요 · 확장 연결: <strong>{{ connectionLabel(status) }}</strong></p>
    <p>Native Messaging: {{ runtime?.registrationCode ? registrationLabel(runtime.registrationCode) : '확인 필요' }}</p>
    <p class="hint">마지막 성공 시각: {{ status.lastSuccessAt ?? '없음' }}. 과거 성공은 현재 연결을 보장하지 않습니다.</p>
    <p v-if="status.lastVerifiedId" class="hint path">마지막 확인 ID {{ status.lastVerifiedId }} · 프로토콜 v1</p>
    <div class="status-row">
      <button type="button" :aria-expanded="expanded" @click="showGuide">확장 프로그램 설치 안내</button>
      <button type="button" :disabled="!runtime?.extensionResourcesAvailable" @click="openFolder">확장 폴더 열기</button>
      <button type="button" @click="copyAddress">Chrome 확장 관리 주소 복사</button>
    </div>
    <div :hidden="!expanded" class="install-guide">
      <ol>
        <li>쓰기가 가능한 최종 보관 위치에 포터블 폴더 전체를 둡니다.</li>
        <li>Chrome 확장 관리에서 개발자 모드를 직접 켭니다.</li>
        <li>[압축해제된 확장 프로그램을 로드합니다]에서 manifest.json이 있는 extension/ 폴더를 선택합니다. ZIP이나 TypeScript 소스 폴더를 선택하지 마세요.</li>
        <li>확장 카드 또는 팝업의 실제 ID를 복사해 아래에 입력하고 호스트를 등록합니다.</li>
        <li>앱 [연결 테스트] 후 30초 안에 확장 팝업의 [연결 테스트]를 누릅니다. 왕복 확인 전에는 연결 완료가 아닙니다.</li>
      </ol>
      <label>Chrome 관리 주소<input readonly value="chrome://extensions" /></label>
      <p class="hint path">확장 경로: {{ runtime?.extensionPath ?? '앱 폴더의 extension/' }}</p>
      <p class="hint">자동 감지를 위해 허용한 사이트의 영상 요청 메타데이터를 관찰합니다. Chrome의 사이트 접근 설정을 존중하며 쿠키·인증 헤더는 수집하지 않습니다.</p>
      <p class="hint">Chrome 미설치·조직 정책 차단 시 설치 또는 관리자 안내를 확인하세요. 웹스토어 강제 설치나 기업 정책 변경은 하지 않습니다.</p>
    </div>
    <p v-if="runtime && !runtime.extensionResourcesAvailable" class="hint">빌드된 확장 리소스가 없습니다. 확장 폴더와 manifest.json이 포함된 배포본이 필요합니다.</p>
    <p v-if="runtime && !runtime.hostAvailable" class="hint">Native Host 실행 파일이 없습니다. 호스트가 포함된 배포본이 필요합니다.</p>
    <p v-if="runtime && !runtime.chromeAvailable" class="hint">표준 설치 경로에서 Chrome을 확인하지 못했습니다. 설치 여부를 직접 확인하세요.</p>
    <label for="extension-id">확장 ID</label>
    <input id="extension-id" class="full-width" :value="extensionId" maxlength="32" autocomplete="off" spellcheck="false" :aria-invalid="extensionId !== '' && !validExtensionId(extensionId)" @input="$emit('update:extensionId', ($event.target as HTMLInputElement).value)" />
    <p v-if="extensionId && !validExtensionId(extensionId)" class="error">확장 ID는 소문자 a~p 32자여야 합니다.</p>
    <p v-if="extensionId !== savedId" class="hint">ID 변경 · 재등록 필요. 폼 수정이나 취소만으로 등록은 바뀌지 않습니다.</p>
    <div v-if="runtime?.registrationCode === 'HOST_PATH_MISMATCH'" class="hint path">
      <p>이전에 등록한 위치 {{ runtime.registeredPath }}</p>
      <p>이 앱의 등록 위치 {{ runtime.manifestPath }}</p>
      <p>확장 폴더도 이동했다면 새 폴더를 Chrome에서 다시 로드하고 실제 ID를 입력하세요. 연결 복구/재등록을 누르면 현재 위치로 등록합니다.</p>
    </div>
    <div class="status-row">
      <button type="button" :disabled="actionsUnavailable || !runtime?.hostAvailable || !validExtensionId(extensionId)" @click="$emit('registration', 'register')">호스트 등록</button>
      <button type="button" :disabled="actionsUnavailable || runtime?.registrationCode !== 'REGISTERED' || extensionId !== savedId" @click="$emit('registration', 'test')">연결 테스트</button>
      <button type="button" :disabled="actionsUnavailable || !runtime?.hostAvailable || !validExtensionId(extensionId)" @click="$emit('registration', 'repair')">연결 복구/재등록</button>
      <button type="button" :disabled="actionsUnavailable || !runtime?.registeredPath" @click="$emit('registration', 'unregister')">연결 해제</button>
    </div>
    <p v-if="!runtime?.connectionActionsAvailable" class="hint">현재는 설치 안내와 ID 저장을 제공합니다. 호스트 등록·왕복 테스트·복구·해제 기능은 아직 사용할 수 없습니다.</p>
    <p v-if="status.phase === 'waiting'" role="status">{{ status.remainingSeconds ?? 30 }}초 안에 확장 팝업의 [연결 테스트]를 누르세요.</p>
    <p v-if="status.errorCode" role="status">{{ registrationLabel(status.errorCode) }}</p>
    <p class="hint">호스트 등록/복구는 확장 ID 저장 및 Windows 사용자별 연결 등록을 즉시 변경합니다. 일반 설정 취소는 이미 실행한 등록/해제를 되돌리지 않습니다.</p>
    <p class="hint">폴더 이동이나 실제 ID 변경 후에는 새 경로로 재등록하고 다시 테스트해야 합니다. 실패 시 폴더 → ID → 호스트 등록 → Chrome 활성화 순서로 확인하세요.</p>
    <p v-if="message" role="status" class="hint">{{ message }}</p>
  </fieldset>
</template>
