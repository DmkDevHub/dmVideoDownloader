<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Settings } from './settings';
import BrowserConnection from './BrowserConnection.vue';
import type { BrowserRuntime, ConnectionStatus } from './browserConnection';

defineProps<{ draft: Settings; busy: boolean; error: string; canSave?: boolean; canRecover?: boolean; notice?: string; savedId?: string; runtime?: BrowserRuntime; status?: ConnectionStatus }>();
defineEmits<{ cancel: []; reset: []; save: []; browse: []; recover: []; registration: [action: 'register' | 'repair' | 'unregister' | 'test'] }>();
const dialog = ref<HTMLDialogElement>();
type ToolStatus = { state: 'ready' | 'missing' | 'invalidPath' | 'failed' | 'timeout' | 'unsupported'; version: string | null };
const toolResult = ref<{ ytDlp: ToolStatus; ffmpeg: ToolStatus }>();
const checkingTools = ref(false);
const toolError = ref('');
async function checkTools() {
  checkingTools.value = true;
  toolError.value = '';
  try { toolResult.value = await invoke('tool_diagnostics'); }
  catch { toolError.value = '도구 상태를 읽지 못했습니다.'; }
  finally { checkingTools.value = false; }
}
function toolLabel(tool: ToolStatus) {
  const labels = { ready: '사용 가능', missing: '파일 없음', invalidPath: '경로 확인 필요', failed: '실행 실패', timeout: '응답 시간 초과', unsupported: '지원 버전 확인 필요' };
  return tool.version ? `${labels[tool.state]} · ${tool.version}` : labels[tool.state];
}
onMounted(() => dialog.value?.showModal());
</script>

<template>
  <dialog ref="dialog" aria-labelledby="settings-title" @cancel.prevent="$emit('cancel')">
    <form @submit.prevent="$emit('save')">
      <header class="app-header"><h2 id="settings-title">설정</h2><button type="button" :disabled="busy" aria-label="설정 닫기" @click="$emit('cancel')">닫기</button></header>
      <fieldset :disabled="busy">
        <legend>일반</legend>
        <label for="download-directory">다운로드 저장 위치</label>
        <div class="input-row"><input id="download-directory" v-model="draft.downloadDirectory" :title="draft.downloadDirectory" autocomplete="off" /><button type="button" @click="$emit('browse')">찾아보기</button></div>
      </fieldset>
      <fieldset :disabled="busy" class="options-grid">
        <legend>다운로드</legend>
        <label>기본 화질<select v-model="draft.quality"><option value="best">최고 화질</option><option value="2160">4K 이하</option><option value="1080">1080p 이하</option><option value="720">720p 이하</option></select></label>
        <label>저장 형식<select v-model="draft.format"><option value="auto">자동</option><option value="mp4">MP4</option><option value="mkv">MKV</option></select></label>
        <label>파일 이름<select v-model="draft.filename"><option value="title">제목</option><option value="title-id">제목-ID</option><option value="date-title">날짜-제목</option></select></label>
        <label>동시 다운로드<select v-model="draft.concurrentDownloads"><option v-for="n in 4" :key="n" :value="n">{{ n }}개</option></select></label>
        <label>전체 다운로드 속도<select :value="draft.speedLimitKib ?? 0" @change="draft.speedLimitKib = Number(($event.target as HTMLSelectElement).value)">
          <option :value="0">무제한</option><option :value="512">512 KiB/s</option>
          <option v-for="n in [1, 2, 5, 10]" :key="n" :value="n * 1024">{{ n }} MiB/s</option>
        </select></label>
        <label>실패 재시도<select v-model="draft.retryCount"><option v-for="n in [0, 1, 3, 5]" :key="n" :value="n">{{ n }}회</option></select></label>
      </fieldset>
      <fieldset :disabled="busy">
        <legend>브라우저 / 알림</legend>
        <label class="check"><input v-model="draft.browserAutoDetect" type="checkbox" />브라우저 영상 자동 감지</label>
        <p class="hint">저장 후 연결 테스트를 완료하면 확장이 이 설정을 읽습니다.</p>
        <label class="check"><input v-model="draft.completionNotification" type="checkbox" />다운로드 완료 알림</label>
      </fieldset>
      <fieldset :disabled="busy || checkingTools">
        <legend>영상 도구</legend>
        <p class="hint">실행 파일 옆 tools 폴더의 yt-dlp.exe와 ffmpeg.exe를 확인합니다.</p>
        <button type="button" @click="checkTools">{{ checkingTools ? '확인 중…' : '도구 확인' }}</button>
        <p v-if="toolResult" role="status">yt-dlp {{ toolLabel(toolResult.ytDlp) }} · FFmpeg {{ toolLabel(toolResult.ffmpeg) }}</p>
        <p v-if="toolError" role="alert">{{ toolError }}</p>
      </fieldset>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <BrowserConnection v-model:extension-id="draft.browserExtensionId" :saved-id="savedId ?? ''" :busy="busy" :can-save="canSave !== false" :runtime="runtime" :status="status" @registration="$emit('registration', $event)" />
      <p v-if="notice" class="error" role="status">{{ notice }}</p>
      <button v-if="canRecover" type="button" :disabled="busy" @click="$emit('recover')">손상 파일 백업 후 기본값 저장</button>
      <p class="hint">다운로드 옵션은 새 작업부터 적용됩니다. 기본값 복원은 저장을 눌러야 적용됩니다.</p>
      <footer class="dialog-actions"><button type="button" :disabled="busy" @click="$emit('reset')">기본값 복원</button><span></span><button type="button" :disabled="busy" @click="$emit('cancel')">취소</button><button class="primary" type="submit" :disabled="busy || canSave === false">{{ busy ? '처리 중…' : '저장' }}</button></footer>
    </form>
  </dialog>
</template>
