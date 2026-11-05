<script setup lang="ts">
import { onMounted } from 'vue';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import SettingsDialog from './SettingsDialog.vue';
import { createSettingsEditor, settingsError, type Settings, type SettingsSnapshot } from './settings';

const address = ref('');
const editor = ref<ReturnType<typeof createSettingsEditor>>();
const startupError = ref('');

onMounted(async () => {
  let smokePassed = false;
  try {
    const snapshot = await invoke<SettingsSnapshot>('load_settings');
    editor.value = createSettingsEditor(snapshot.settings, snapshot.defaults);
    if (import.meta.env.MODE === 'smoke') {
      const changed = { ...snapshot.settings, quality: '1080' as const };
      await invoke('save_settings', { settings: changed });
      const loaded = await invoke<SettingsSnapshot>('load_settings');
      smokePassed = loaded.settings.quality === '1080';
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
      <div class="status-row"><span class="badge">Chrome 연동: 확인 필요</span><span>자동 감지: {{ editor ? (editor.state.current.browserAutoDetect ? '켜짐 · 연결 후 동기화 필요' : '꺼짐') : '설정 읽는 중' }}</span></div>
    </section>
    <section class="panel" aria-labelledby="candidates-heading">
      <h2 id="candidates-heading">감지된 영상</h2>
      <div class="table-scroll"><table><thead><tr><th>제목 / 사이트</th><th>종류</th><th>화질</th><th>분석 상태</th><th>작업</th></tr></thead><tbody><tr><td colspan="5" class="empty">아직 감지된 영상이 없습니다. Chrome 감지 기능은 준비 중입니다.</td></tr></tbody></table></div>
    </section>
    <section class="panel" aria-labelledby="jobs-heading">
      <h2 id="jobs-heading">다운로드</h2>
      <div class="table-scroll"><table><thead><tr><th>제목</th><th>화질</th><th>진행률 / 속도</th><th>상태</th><th>작업</th></tr></thead><tbody><tr><td colspan="5" class="empty">진행 중인 다운로드가 없습니다.</td></tr></tbody></table></div>
    </section>
    <footer class="status-row"><button disabled>다운로드 폴더 열기</button><p class="hint" role="status">브라우저 연결이 설정되지 않았습니다. 감지·다운로드 기능은 준비 중입니다.</p></footer>
    <p v-if="startupError" class="error" role="alert">{{ startupError }}</p>
    <SettingsDialog v-if="editor?.state.open" :draft="editor.state.draft" :busy="editor.state.busy" :error="editor.state.error" @cancel="editor.cancel()" @reset="editor.reset()" @browse="editor.browse(() => invoke<string | null>('pick_download_directory'))" @save="editor.save((settings: Settings) => invoke<Settings>('save_settings', { settings }))" />
  </main>
</template>
