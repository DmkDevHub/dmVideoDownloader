<script setup lang="ts">
import { onMounted, ref } from 'vue';
import type { Settings } from './settings';

defineProps<{ draft: Settings; busy: boolean; error: string }>();
defineEmits<{ cancel: []; reset: []; save: []; browse: [] }>();
const dialog = ref<HTMLDialogElement>();
onMounted(() => dialog.value?.showModal());
</script>

<template>
  <dialog ref="dialog" aria-labelledby="settings-title" @cancel.prevent="$emit('cancel')">
    <form @submit.prevent="$emit('save')">
      <header class="app-header"><h2 id="settings-title">설정</h2><button type="button" :disabled="busy" aria-label="설정 닫기" @click="$emit('cancel')">닫기</button></header>
      <fieldset :disabled="busy">
        <legend>일반</legend>
        <label for="download-directory">다운로드 저장 위치</label>
        <div class="input-row"><input id="download-directory" v-model="draft.downloadDirectory" :title="draft.downloadDirectory" autocomplete="off" /><button type="button" disabled @click="$emit('browse')">찾아보기</button></div>
      </fieldset>
      <fieldset :disabled="busy" class="options-grid">
        <legend>다운로드</legend>
        <label>기본 화질<select v-model="draft.quality"><option value="best">최고 화질</option><option value="2160">4K 이하</option><option value="1080">1080p 이하</option><option value="720">720p 이하</option></select></label>
        <label>저장 형식<select v-model="draft.format"><option value="auto">자동</option><option value="mp4">MP4</option><option value="mkv">MKV</option></select></label>
        <label>파일 이름<select v-model="draft.filename"><option value="title">제목</option><option value="title-id">제목-ID</option><option value="date-title">날짜-제목</option></select></label>
        <label>동시 다운로드<select v-model="draft.concurrentDownloads"><option v-for="n in 4" :key="n" :value="n">{{ n }}개</option></select></label>
        <label>실패 재시도<select v-model="draft.retryCount"><option v-for="n in [0, 1, 3, 5]" :key="n" :value="n">{{ n }}회</option></select></label>
      </fieldset>
      <fieldset :disabled="busy">
        <legend>브라우저 / 알림</legend>
        <label class="check"><input v-model="draft.browserAutoDetect" type="checkbox" />브라우저 영상 자동 감지</label>
        <p class="hint">확장 연결 후 동기화됩니다. 이 옵션만으로 확장을 설치하거나 등록하지 않습니다.</p>
        <label class="check"><input v-model="draft.completionNotification" type="checkbox" />다운로드 완료 알림</label>
      </fieldset>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <p class="hint">다운로드 옵션은 새 작업부터 적용됩니다. 설정 저장 기능은 준비 중입니다.</p>
      <footer class="dialog-actions"><button type="button" :disabled="busy" @click="$emit('reset')">기본값 복원</button><span></span><button type="button" :disabled="busy" @click="$emit('cancel')">취소</button><button class="primary" type="submit" disabled>저장</button></footer>
    </form>
  </dialog>
</template>
