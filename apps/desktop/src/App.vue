<script setup lang="ts">
import { onMounted, onUnmounted, nextTick } from 'vue';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import SettingsDialog from './SettingsDialog.vue';
import { createSettingsEditor, settingsError, type Settings, type SettingsSnapshot } from './settings';
import { connectionLabel, registrationLabel, type BrowserRuntime, type ConnectionStatus } from './browserConnection';

const address = ref('');
const editor = ref<ReturnType<typeof createSettingsEditor>>();
const startupError = ref('');
const snapshotState = ref<SettingsSnapshot>();
const runtime = ref<BrowserRuntime>();
const actionMessage = ref('');
const connection = ref<ConnectionStatus>({ phase: 'needsCheck', roundTripConfirmed: false, lastSuccessAt: null });
async function refreshConnection() {
  try { connection.value = await invoke<ConnectionStatus>('browser_connection_status'); }
  catch { connection.value = { ...connection.value, phase: 'needsCheck', roundTripConfirmed: false }; }
}
const candidates = ref<{ id: string; title: string; sourceType: string }[]>([]);
let candidateTimer: ReturnType<typeof setInterval> | undefined;
let refreshingCandidates = false;
async function refreshCandidates() {
  if (refreshingCandidates) return;
  refreshingCandidates = true;
  try { candidates.value = await invoke('received_candidates'); }
  catch { /* Retain the last known inbox on a transient IPC error. */ }
  finally { refreshingCandidates = false; }
}
onUnmounted(() => clearInterval(candidateTimer));

async function browserRegistration(action: 'register' | 'repair' | 'unregister' | 'test') {
  const current = editor.value;
  if (!current || current.state.busy) return;
  current.state.busy = true;
  current.state.error = '';
  try {
    if (action === 'test') {
      connection.value = await invoke<ConnectionStatus>('browser_begin_test');
    } else {
      await invoke(action === 'unregister' ? 'browser_unregister' : 'browser_register', { extensionId: current.state.draft.browserExtensionId, repair: action === 'repair' });
      actionMessage.value = action === 'unregister' ? '호스트 등록을 해제했습니다.' : '호스트를 등록했습니다. 연결 테스트가 필요합니다.';
    }
  } catch (error) { current.state.error = registrationLabel(error); }
  finally {
    try {
      runtime.value = await invoke<BrowserRuntime>('browser_runtime');
      if (runtime.value.configuredId !== undefined) current.state.current.browserExtensionId = runtime.value.configuredId;
    } catch { runtime.value = undefined; }
    await refreshConnection();
    current.state.busy = false;
  }
}

async function openDownloadFolder() {
  try { await invoke('open_download_folder'); actionMessage.value = '다운로드 폴더 열기를 요청했습니다.'; }
  catch { actionMessage.value = '다운로드 폴더를 열 수 없습니다. 저장 위치·설정 파일·권한을 확인하세요.'; }
}

function applySnapshot(snapshot: SettingsSnapshot) {
  snapshotState.value = snapshot;
  editor.value = createSettingsEditor(snapshot.settings, snapshot.defaults);
  startupError.value = '';
}
async function readSettings() {
  try { applySnapshot(await invoke<SettingsSnapshot>('load_settings')); }
  catch (error) { startupError.value = settingsError(error); }
}
async function recoverSettings() {
  if (!editor.value || editor.value.state.busy || !snapshotState.value?.canRecover) return;
  editor.value.state.busy = true;
  try {
    applySnapshot(await invoke<SettingsSnapshot>('recover_settings'));
    editor.value?.open();
  } catch (error) {
    if (editor.value) editor.value.state.error = settingsError(error);
  } finally { if (editor.value) editor.value.state.busy = false; }
}

onMounted(async () => {
  let smokePassed = false;
  try {
    const snapshot = await invoke<SettingsSnapshot>('load_settings');
    applySnapshot(snapshot);
    try { runtime.value = await invoke<BrowserRuntime>('browser_runtime'); } catch { /* Keep unknown status on diagnostic failure. */ }
    if (import.meta.env.MODE === 'smoke') {
      const phase = await invoke<string>('smoke_phase');
      if (phase === 'write') {
        editor.value!.open();
        Object.assign(editor.value!.state.draft, { quality: '1080', browserAutoDetect: false,
          downloadDirectory: snapshot.defaults.downloadDirectory + ' 한글' });
        await editor.value!.save((settings) => invoke<Settings>('save_settings', { settings }));
        const loaded = await invoke<SettingsSnapshot>('load_settings');
        smokePassed = !editor.value!.state.error && loaded.settings.quality === '1080'
          && !loaded.settings.browserAutoDetect && loaded.settings.downloadDirectory === snapshot.defaults.downloadDirectory + ' 한글';
      } else if (phase === 'restart') {
        smokePassed = snapshot.canSave && snapshot.settings.quality === '1080' && !snapshot.settings.browserAutoDetect
          && snapshot.settings.downloadDirectory === snapshot.defaults.downloadDirectory + ' 한글';
      } else if (phase === 'corrupt') {
        const protectedOriginal = !snapshot.canSave && snapshot.canRecover;
        const recovered = await invoke<SettingsSnapshot>('recover_settings');
        smokePassed = protectedOriginal && recovered.canSave && recovered.settings.quality === 'best';
        applySnapshot(recovered);
      } else if (phase === 'bridge' || phase === 'chrome-bridge') {
        if (phase === 'chrome-bridge') {
          await invoke('browser_register', { extensionId: snapshot.settings.browserExtensionId });
          runtime.value = await invoke<BrowserRuntime>('browser_runtime');
          if (runtime.value.registrationCode !== 'REGISTERED') throw new Error('REGISTRATION_FAILED');
          connection.value = await invoke<ConnectionStatus>('browser_begin_test');
          if (connection.value.phase !== 'waiting') throw new Error('CONNECTION_TEST_NOT_WAITING');
        }
        for (let attempt = 0; attempt < 100 && !smokePassed; attempt++) {
          await refreshCandidates();
          await nextTick();
          smokePassed = await invoke<boolean>('smoke_bridge_received') && document.querySelectorAll('[data-received-candidate]').length === 1;
          if (phase === 'chrome-bridge') {
            await refreshConnection();
            smokePassed = smokePassed && connection.value.phase === 'disconnected' && !!connection.value.lastSuccessAt;
          }
          if (!smokePassed) await new Promise(resolve => setTimeout(resolve, 100));
        }
        if (phase === 'chrome-bridge') {
          await invoke('browser_unregister');
          runtime.value = await invoke<BrowserRuntime>('browser_runtime');
        }
      } else if (phase === 'future') {
        smokePassed = !snapshot.canSave && !snapshot.canRecover && snapshot.notice === 'UNSUPPORTED_SETTINGS_VERSION';
      }
      smokePassed = smokePassed && runtime.value?.connectionActionsAvailable === true && runtime.value.registrationCode !== 'REGISTERED';
      // Exercise the internal model in a hidden WebView, without clicks or keyboard events.
      editor.value?.open();
      await nextTick();
      smokePassed = smokePassed && document.querySelectorAll('dialog').length === 1
        && document.querySelector<HTMLDialogElement>('dialog')?.open === true
        && document.querySelector('#extension-id') !== null;
      editor.value?.reset();
      editor.value?.cancel();
      await nextTick();
      smokePassed = smokePassed && document.querySelector('dialog') === null;
    } else {
      if (snapshot.notice) editor.value?.open();
      void refreshCandidates();
      void refreshConnection();
      candidateTimer = setInterval(() => { void refreshCandidates(); void refreshConnection(); }, 750);
    }
  } catch (error) {
    startupError.value = settingsError(error);
  }
  // Compiled out of normal builds.
  if (import.meta.env.MODE === 'smoke') {
    await invoke('smoke_ready', {
      mounted: document.querySelector('[data-app-ready]') !== null && !!editor.value && smokePassed,
    });
  }
});
</script>

<template>
  <main data-app-ready>
    <header class="app-header">
      <div><p class="eyebrow">개인용 · Windows 포터블</p><h1>영상 다운로더</h1></div>
      <button type="button" :disabled="!editor" @click="editor?.open()">설정</button>
    </header>
    <section class="panel" aria-label="영상 주소">
      <label for="video-url">영상 URL 직접 입력</label>
      <div class="input-row"><input id="video-url" v-model="address" type="url" placeholder="공개 영상 주소를 입력하세요" autocomplete="off" spellcheck="false" /><button disabled>분석</button></div>
      <p class="hint">인증 없이 접근 가능한 공개 영상만 지원합니다. 분석 기능은 준비 중입니다.</p>
      <div class="status-row"><span class="badge">Chrome 연동: {{ connectionLabel(connection) }}</span><span>자동 감지 설정: {{ editor && snapshotState?.canSave ? (editor.state.current.browserAutoDetect ? '켜짐' : '꺼짐') : '설정 확인 필요' }}</span></div>
    </section>
    <section class="panel" aria-labelledby="candidates-heading">
      <h2 id="candidates-heading">감지된 영상</h2>
      <div class="table-scroll"><table><thead><tr><th>제목</th><th>종류</th><th>화질</th><th>분석 상태</th></tr></thead><tbody>
        <tr v-if="!candidates.length"><td colspan="4" class="empty">Chrome 확장 팝업에서 선택한 영상이 여기에 표시됩니다.</td></tr>
        <tr v-for="candidate in candidates" :key="candidate.id" data-received-candidate><td>{{ candidate.title }}</td><td>{{ candidate.sourceType.toUpperCase() }}</td><td>미확인</td><td>후보 수신 · 분석 대기</td></tr>
      </tbody></table></div>
    </section>
    <section class="panel" aria-labelledby="jobs-heading">
      <h2 id="jobs-heading">다운로드</h2>
      <div class="table-scroll"><table><thead><tr><th>제목</th><th>화질</th><th>진행률 / 속도</th><th>상태</th><th>작업</th></tr></thead><tbody><tr><td colspan="5" class="empty">진행 중인 다운로드가 없습니다.</td></tr></tbody></table></div>
    </section>
    <footer class="status-row"><button :disabled="!snapshotState?.canSave" @click="openDownloadFolder">다운로드 폴더 열기</button><p class="hint" role="status">{{ actionMessage }}</p></footer>
    <div v-if="startupError" class="error" role="alert">{{ startupError }} <button @click="readSettings">설정 다시 읽기</button></div>
    <SettingsDialog v-if="editor?.state.open" :draft="editor.state.draft" :busy="editor.state.busy" :error="editor.state.error" :can-save="snapshotState?.canSave" :can-recover="snapshotState?.canRecover" :notice="snapshotState?.notice ? settingsError(snapshotState.notice) : ''" :saved-id="editor.state.current.browserExtensionId" :runtime="runtime" :status="connection" @cancel="editor.cancel()" @reset="editor.reset()" @recover="recoverSettings" @registration="browserRegistration" @browse="editor.browse(() => invoke<string | null>('pick_download_directory'))" @save="editor.save((settings: Settings) => invoke<Settings>('save_settings', { settings }))" />
  </main>
</template>
