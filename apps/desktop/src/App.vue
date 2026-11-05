<script setup lang="ts">
import { onMounted } from 'vue';

// Included only in the separate smoke build; normal builds do not call test IPC.
if (import.meta.env.MODE === 'smoke') {
  onMounted(async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('smoke_ready', {
      mounted: document.querySelector('[data-app-ready]') !== null,
    });
  });
}
</script>

<template>
  <main data-app-ready>
    <h1>영상 다운로더</h1>
    <p>앱 실행 골격이 준비되었습니다.</p>
    <p>영상 감지·다운로드 기능은 아직 구현되지 않았습니다.</p>
  </main>
</template>
