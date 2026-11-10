<script setup lang="ts">
import { onMounted, onUnmounted, nextTick } from 'vue';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import SettingsDialog from './SettingsDialog.vue';
import { createSettingsEditor, settingsError, type Settings, type SettingsSnapshot } from './settings';
import { connectionLabel, registrationLabel, type BrowserRuntime, type ConnectionStatus } from './browserConnection';
import { analysisError, analysisSource, resolution, trackLabel, type AnalysisReport } from './analysis';
import { downloadError, downloadPhase, progressLabel, type DownloadAction, type DownloadJob, type DownloadRequest } from './download';

const address = ref('');
const analysis = ref<AnalysisReport>();
const analysisBusy = ref(false);
const analysisMessage = ref('');
const analyzingCandidate = ref<string>();
const downloadSource = ref<DownloadRequest>();
const submittingDownload = ref(false);
const jobs = ref<DownloadJob[]>([]);
const downloadMessage = ref('');
let jobsRequest = 0;
async function refreshDownloads() {
  const request = ++jobsRequest;
  try {
    const items = await invoke<DownloadJob[]>('download_jobs');
    if (request === jobsRequest) jobs.value = items;
  } catch { /* Keep the last known job state on a transient IPC failure. */ }
}
async function startDownload() {
  const request = downloadSource.value;
  if (!request || !analysis.value || submittingDownload.value) return;
  submittingDownload.value = true;
  downloadMessage.value = '';
  try {
    const id = await invoke<number>('download_video', { request });
    await refreshDownloads();
    return id;
  }
  catch (error) { downloadMessage.value = downloadError(error); }
  finally { submittingDownload.value = false; }
}
async function changeDownload(id: number, action: DownloadAction) {
  downloadMessage.value = '';
  try { await invoke('download_action', { id, action }); }
  catch (error) { downloadMessage.value = downloadError(error); }
  await refreshDownloads();
}
async function runAnalysis(command: 'analyze_url' | 'analyze_candidate', args: Record<string, string>) {
  if (analysisBusy.value) return;
  analysisBusy.value = true;
  analyzingCandidate.value = args.id;
  analysis.value = undefined;
  downloadSource.value = undefined;
  analysisMessage.value = '';
  try {
    analysis.value = await invoke<AnalysisReport>(command, args);
    downloadSource.value = command === 'analyze_url' ? { kind: 'direct', address: args.address } : { kind: 'candidate', id: args.id };
  }
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
  let smokeDetails = '';
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
      } else if (['bridge', 'bridge-analysis', 'chrome-move-from', 'chrome-move-to'].includes(phase)) {
        const chromeTest = phase === 'chrome-move-from' || phase === 'chrome-move-to';
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
        if (phase === 'bridge-analysis' && smokePassed) {
          await runAnalysis('analyze_candidate', { id: candidates.value[0].id });
          await nextTick();
          const candidateShown = analysis.value?.source === 'media' &&
            document.querySelectorAll('[data-analysis-result] tbody tr').length === 1;
          const first = await startDownload();
          const firstError = downloadMessage.value;
          await invoke('save_settings', { settings: { ...snapshot.settings, completionNotification: false } });
          const second = await startDownload();
          const secondError = downloadMessage.value;
          const secondWasActive = jobs.value.some(job => job.id === second && ['queued', 'analyzing', 'downloading'].includes(job.phase));
          await invoke('save_settings', { settings: { ...snapshot.settings, completionNotification: false,
            downloadDirectory: snapshot.settings.downloadDirectory + ' HLS', format: 'mkv', quality: '720' } });
          address.value = 'https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8';
          await analyzeAddress();
          const streamAnalysisError = analysisMessage.value;
          const hls = await startDownload();
          const streamDownloadError = downloadMessage.value;
          address.value = 'https://mdn.github.io/learning-area/html/multimedia-and-embedding/video-and-audio-content/extra-video-features.html';
          await analyzeAddress();
          await nextTick();
          const directShown = (analysis.value as AnalysisReport | undefined)?.source === 'direct' &&
            document.querySelectorAll('[data-analysis-result] tbody tr').length >= 2;
          address.value = 'http://127.0.0.1/private';
          await analyzeAddress();
          await nextTick();
          let downloadsShown = false;
          for (let attempt = 0; first && second && hls && attempt < 240 && !downloadsShown; attempt++) {
            await refreshDownloads();
            await nextTick();
            const saved = jobs.value.filter(job => job.phase === 'completed');
            if (jobs.value.some(job => job.phase === 'failed')) break;
            downloadsShown = secondWasActive && new Set([first, second, hls]).size === 3 && saved.length === 3 &&
              saved.every(job => job.filename?.endsWith(job.id === hls ? '.mkv' : '.mp4')) &&
              saved.some(job => job.id === hls && job.height === 576) &&
              new Set(saved.map(job => job.filename)).size === 3 &&
              document.querySelectorAll('[data-download-row]').length === 3 &&
              await invoke<number>('smoke_notification_count') === 1;
            if (!downloadsShown) await new Promise(resolve => setTimeout(resolve, 250));
          }
          smokeDetails = JSON.stringify({ candidateShown, directShown, first, second, firstError, secondError, secondWasActive,
            hls, streamAnalysisError, streamDownloadError,
            jobs: jobs.value.map(({ phase, error }) => ({ phase, error })),
            notifications: await invoke<number>('smoke_notification_count') });
          smokePassed = candidateShown && directShown && !analysis.value &&
            document.querySelector('[data-analysis-message]')?.textContent?.trim() === analysisError('URL_NOT_ALLOWED') && downloadsShown;
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
      void refreshDownloads();
      candidateTimer = setInterval(() => { void refreshCandidates(); void refreshConnection(); void refreshDownloads(); }, 500);
    }
  } catch (error) {
    startupError.value = settingsError(error);
    smokeDetails = startupError.value;
  }
  // Compiled out of normal builds.
  if (import.meta.env.MODE === 'smoke') {
    await invoke('smoke_ready', {
      mounted: document.querySelector('[data-app-ready]') !== null && !!editor.value && smokePassed,
      diagnostics: smokeDetails,
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
    <p v-if="analysisMessage" role="alert" class="error" data-analysis-message>{{ analysisMessage }}</p>
    <section v-if="analysis" class="panel" aria-label="포맷 분석 결과" data-analysis-result>
      <h2>{{ analysis.title }}</h2>
      <p class="hint" role="status">{{ analysisSource[analysis.source] }}에서 포맷 {{ analysis.formats.length }}개를 확인했습니다.</p>
      <p v-if="analysis.excludedProtected" class="hint">보호 가능성이 있는 포맷 {{ analysis.excludedProtected }}개를 제외했습니다.</p>
      <button type="button" :disabled="submittingDownload || !snapshotState?.canSave" @click="startDownload">다운로드</button>
      <div class="table-scroll"><table><thead><tr><th>포맷</th><th>형식</th><th>해상도</th><th>트랙</th><th>영상 코덱</th><th>음성 코덱</th></tr></thead><tbody>
        <tr v-for="format in analysis.formats" :key="format.id">
          <td>{{ format.id }}</td><td>{{ format.container ?? '미확인' }}</td><td>{{ resolution(format) }}</td><td>{{ trackLabel[format.tracks] }}</td>
          <td>{{ format.tracks === 'audioOnly' ? '없음' : (format.videoCodec ?? '미확인') }}</td>
          <td>{{ format.tracks === 'videoOnly' ? '없음' : (format.audioCodec ?? '미확인') }}</td>
        </tr>
      </tbody></table></div>
    </section>
    <section class="panel" aria-labelledby="jobs-heading">
      <h2 id="jobs-heading">다운로드</h2>
      <div class="table-scroll"><table><thead><tr><th>제목</th><th>화질</th><th>진행률 / 속도</th><th>상태</th><th>작업</th></tr></thead><tbody>
        <tr v-for="job in jobs" :key="job.id" data-download-row>
          <td>{{ job.title }}</td><td>{{ job.height ? job.height + 'p' : '미확인' }}</td>
          <td>{{ job.phase === 'completed' ? '100%' : progressLabel(job.progress) }}</td>
          <td>{{ downloadPhase[job.phase] }}<span v-if="job.phase === 'retrying'"> ({{ job.attempt + 1 }}회차)</span><span v-if="job.error" class="error"> · {{ downloadError(job.error) }}</span></td>
          <td>
            <span v-if="job.filename">{{ job.filename }}</span>
            <button v-if="['queued', 'analyzing', 'downloading', 'retrying'].includes(job.phase)" type="button" @click="changeDownload(job.id, 'pause')">일시정지</button>
            <button v-if="job.phase === 'paused'" type="button" @click="changeDownload(job.id, 'resume')">재개</button>
            <button v-if="job.canRetry || ['queued', 'analyzing', 'downloading', 'merging', 'retrying', 'pausing', 'paused'].includes(job.phase)" type="button" @click="changeDownload(job.id, 'cancel')">취소</button>
            <button v-if="job.canRetry" type="button" @click="changeDownload(job.id, 'retry')">다시 시도</button>
          </td>
        </tr>
        <tr v-if="!jobs.length"><td colspan="5" class="empty">진행 중인 다운로드가 없습니다.</td></tr>
      </tbody></table></div>
      <p v-if="downloadMessage" role="alert" class="error">{{ downloadMessage }}</p>
    </section>
    <footer class="status-row"><button :disabled="!snapshotState?.canSave" @click="openDownloadFolder">다운로드 폴더 열기</button><p class="hint" role="status">{{ actionMessage }}</p></footer>
    <div v-if="startupError" class="error" role="alert">{{ startupError }} <button @click="readSettings">설정 다시 읽기</button></div>
    <SettingsDialog v-if="editor?.state.open" :draft="editor.state.draft" :busy="editor.state.busy" :error="editor.state.error" :can-save="snapshotState?.canSave" :can-recover="snapshotState?.canRecover" :notice="snapshotState?.notice ? settingsError(snapshotState.notice) : ''" :saved-id="editor.state.current.browserExtensionId" :runtime="runtime" :status="connection" @cancel="editor.cancel()" @reset="editor.reset()" @recover="recoverSettings" @registration="browserRegistration" @browse="editor.browse(() => invoke<string | null>('pick_download_directory'))" @save="persistSettings" />
  </main>
</template>
