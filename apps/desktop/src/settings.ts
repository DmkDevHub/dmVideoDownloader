import { reactive } from 'vue';

export interface Settings {
  schemaVersion: 1;
  downloadDirectory: string;
  quality: 'best' | '2160' | '1080' | '720';
  format: 'auto' | 'mp4' | 'mkv';
  filename: 'title' | 'title-id' | 'date-title';
  concurrentDownloads: number;
  retryCount: 0 | 1 | 3 | 5;
  browserAutoDetect: boolean;
  browserExtensionId: string;
  completionNotification: boolean;
}

export function defaultSettings(downloadDirectory: string): Settings {
  return { schemaVersion: 1, downloadDirectory, quality: 'best', format: 'auto',
    filename: 'title', concurrentDownloads: 2, retryCount: 3,
    browserAutoDetect: true, browserExtensionId: '', completionNotification: true };
}

export function createSettingsEditor(initial: Settings, defaults: Settings) {
  const state = reactive({ open: false, current: { ...initial }, draft: { ...initial }, error: '', busy: false });
  return {
    state,
    open() { if (!state.open) { state.draft = { ...state.current }; state.error = ''; state.open = true; } },
    cancel() { if (!state.busy) { state.draft = { ...state.current }; state.error = ''; state.open = false; } },
    reset() { if (!state.busy) state.draft = { ...defaults }; },
  };
}
