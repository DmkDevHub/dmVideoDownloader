<script setup lang="ts">
import { onMounted, onUnmounted, nextTick } from 'vue';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import SettingsDialog from './SettingsDialog.vue';
import { createSettingsEditor, settingsError, type Settings, type SettingsSnapshot } from './settings';
import { connectionLabel, registrationLabel, type BrowserRuntime, type ConnectionStatus } from './browserConnection';
import { analysisError, analysisSource, type AnalysisReport } from './analysis';

const address = ref('');
const analysis = ref<AnalysisReport>();
const analysisBusy = ref(false);
const analysisMessage = ref('');
const analyzingCandidate = ref<string>();
async function runAnalysis(command: 'analyze_url' | 'analyze_candidate', args: Record<string, string>) {
  if (analysisBusy.value) return;
  analysisBusy.value = true;
  analyzingCandidate.value = args.id;
  analysis.value = undefined;
  analysisMessage.value = '';
  try { analysis.value = await invoke<AnalysisReport>(command, args); }
  catch (error) { analysisMessage.value = analysisError(error); }
  finally { analysisBusy.value = false; analyzingCandidate.value = undefined; }
}
function analyzeAddress() {
  const input = address.value.trim();
  if (input) return runAnalysis('analyze_url', { address: input });
}
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
  try { applySnapshot(await invoke<SettingsSnapshot>('load_settings')); await refreshBrowserRuntime(); }
  catch (error) { startupError.value = settingsError(error); }
}
async function refreshBrowserRuntime() {
  try { runtime.value = await invoke<BrowserRuntime>('browser_runtime'); }
  catch { runtime.value = undefined; }
}
function openSettings() {
  editor.value?.open();
  void refreshBrowserRuntime();
}
async function persistSettings() {
  await editor.value?.save((settings: Settings) => invoke<Settings>('save_settings', { settings }));
  await refreshBrowserRuntime();
  await refreshConnection();
}
async function recoverSettings() {
  if (!editor.value || editor.value.state.busy || !snapshotState.value?.canRecover) return;
  editor.value.state.busy = true;
  try {
    applySnapshot(await invoke<SettingsSnapshot>('recover_settings'));
    editor.value?.open();
    await refreshBrowserRuntime();
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
      } else if (phase === 'bridge' || phase === 'chrome-move-from' || phase === 'chrome-move-to') {
        const chromeTest = phase !== 'bridge';
        if (chromeTest) {
          const repair = phase === 'chrome-move-to';
          if (repair && runtime.value?.registrationCode !== 'HOST_PATH_MISMATCH') throw new Error('HOST_PATH_MISMATCH');
          await invoke('browser_register', { extensionId: snapshot.settings.browserExtensionId, repair });
          runtime.value = await invoke<BrowserRuntime>('browser_runtime');
          if (runtime.value.registrationCode !== 'REGISTERED') throw new Error('REGISTRATION_FAILED');
          connection.value = await invoke<ConnectionStatus>('browser_begin_test');
          if (connection.value.phase !== 'waiting') throw new Error('CONNECTION_TEST_NOT_WAITING');
        }
        for (let attempt = 0; attempt < 100 && !smokePassed; attempt++) {
          await refreshCandidates();
          await nextTick();
          smokePassed = await invoke<boolean>('smoke_bridge_received') && document.querySelectorAll('[data-received-candidate]').length === 1;
          if (chromeTest) {
            await refreshConnection();
            smokePassed = smokePassed && connection.value.phase === 'disconnected' && !!connection.value.lastSuccessAt;
          }
          if (!smokePassed) await new Promise(resolve => setTimeout(resolve, 100));
        }
        if (phase === 'chrome-move-to') {
          await invoke('browser_unregister');
          runtime.value = await invoke<BrowserRuntime>('browser_runtime');
        }
      } else if (phase === 'future') {
        smokePassed = !snapshot.canSave && !snapshot.canRecover && snapshot.notice === 'UNSUPPORTED_SETTINGS_VERSION';
      }
      smokePassed = smokePassed && runtime.value?.connectionActionsAvailable === true &&
        (phase === 'chrome-move-from' ? runtime.value.registrationCode === 'REGISTERED' : runtime.value.registrationCode !== 'REGISTERED');
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
      <button type="button" :disabled="!editor" @click="openSettings">설정</button>
    </header>
    <section class="panel" aria-label="영상 주소">
      <label for="video-url">영상 URL 직접 입력</label>
      <form class="input-row" @submit.prevent="analyzeAddress"><input id="video-url" v-model="address" :disabled="analysisBusy" type="url" placeholder="공개 영상 주소를 입력하세요" autocomplete="off" spellcheck="false" maxlength="8192" /><button :disabled="analysisBusy || !address.trim()">{{ analysisBusy ? '분석 중…' : '분석' }}</button></form>
      <p class="hint">공개 영상의 포맷을 확인합니다.</p>
      <div class="status-row"><span class="badge">Chrome 연동: {{ connectionLabel(connection) }}</span><span>자동 감지 설정: {{ editor && snapshotState?.canSave ? (editor.state.current.browserAutoDetect ? '켜짐' : '꺼짐') : '설정 확인 필요' }}</span></div>
    </section>
    <section class="panel" aria-labelledby="candidates-heading">
      <h2 id="candidates-heading">감지된 영상</h2>
      <div class="table-scroll"><table><thead><tr><th>제목</th><th>종류</th><th>화질</th><th>작업</th></tr></thead><tbody>
        <tr v-if="!candidates.length"><td colspan="4" class="empty">Chrome 확장 팝업에서 선택한 영상이 여기에 표시됩니다.</td></tr>
        <tr v-for="candidate in candidates" :key="candidate.id" data-received-candidate><td>{{ candidate.title }}</td><td>{{ candidate.sourceType.toUpperCase() }}</td><td>미확인</td><td><button type="button" :disabled="analysisBusy" @click="runAnalysis('analyze_candidate', { id: candidate.id })">{{ analyzingCandidate === candidate.id ? '분석 중…' : '분석' }}</button></td></tr>
      </tbody></table></div>
    </section>
    <p v-if="analysisMessage" role="alert" class="error">{{ analysisMessage }}</p>
    <section v-if="analysis" class="panel" aria-label="포맷 분석 결과" data-analysis-result>
      <h2>{{ analysis.title }}</h2>
      <p class="hint" role="status">{{ analysisSource[analysis.source] }}에서 포맷 {{ analysis.formats.length }}개를 확인했습니다.</p>
      <div class="table-scroll"><table><thead><tr><th>포맷</th><th>형식</th><th>화질</th></tr></thead><tbody>
        <tr v-for="(format, index) in analysis.formats" :key="`${format.id}-${index}`"><td>{{ format.id }}</td><td>{{ format.container ?? '미확인' }}</td><td>{{ format.height ? `${format.height}p` : '미확인' }}</td></tr>
      </tbody></table></div>
    </section>
    <section class="panel" aria-labelledby="jobs-heading">
      <h2 id="jobs-heading">다운로드</h2>
      <div class="table-scroll"><table><thead><tr><th>제목</th><th>화질</th><th>진행률 / 속도</th><th>상태</th><th>작업</th></tr></thead><tbody><tr><td colspan="5" class="empty">진행 중인 다운로드가 없습니다.</td></tr></tbody></table></div>
    </section>
    <footer class="status-row"><button :disabled="!snapshotState?.canSave" @click="openDownloadFolder">다운로드 폴더 열기</button><p class="hint" role="status">{{ actionMessage }}</p></footer>
    <div v-if="startupError" class="error" role="alert">{{ startupError }} <button @click="readSettings">설정 다시 읽기</button></div>
    <SettingsDialog v-if="editor?.state.open" :draft="editor.state.draft" :busy="editor.state.busy" :error="editor.state.error" :can-save="snapshotState?.canSave" :can-recover="snapshotState?.canRecover" :notice="snapshotState?.notice ? settingsError(snapshotState.notice) : ''" :saved-id="editor.state.current.browserExtensionId" :runtime="runtime" :status="connection" @cancel="editor.cancel()" @reset="editor.reset()" @recover="recoverSettings" @registration="browserRegistration" @browse="editor.browse(() => invoke<string | null>('pick_download_directory'))" @save="persistSettings" />
  </main>
</template>
