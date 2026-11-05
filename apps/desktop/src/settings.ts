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
export interface SettingsSnapshot { settings: Settings; defaults: Settings; notice: string | null; canSave: boolean; canRecover: boolean }

export function settingsError(error: unknown): string {
  const code = typeof error === 'string' ? error : '';
  const messages: Record<string, string> = {
    SETTINGS_CHANGED: '다른 앱에서 설정이 변경되었습니다. 앱을 재시작한 뒤 다시 편집하세요.',
    SETTINGS_BUSY: '다른 앱이 설정을 사용 중입니다. 잠시 후 다시 시도하세요.',
    INVALID_EXTENSION_ID: '확장 ID는 소문자 a~p 32자여야 합니다.',
    UNSUPPORTED_SETTINGS_VERSION: '지원하지 않는 설정 버전입니다. 원본 파일을 보존하고 호환되는 앱을 사용하세요.',
    INVALID_SETTINGS: '설정 파일이 손상되었거나 형식이 맞지 않습니다. 원본을 보존했습니다. 백업 후 기본값으로 복구할 수 있습니다.',
    SETTINGS_RECOVERED: '원본은 data 폴더의 settings.backup-*.json에 보관하고 기본값으로 복구했습니다.',
    SETTINGS_NOT_WRITABLE: '앱 폴더에 설정을 쓸 수 없습니다. 포터블 폴더 전체를 쓰기 가능한 위치로 옮겨 실행하세요.',
    BACKUP_FAILED: '원본 설정 백업에 실패했습니다. 기존 설정을 보존하고 복구를 중단했습니다.',
  };
  return messages[code] ?? '설정을 저장하거나 읽을 수 없습니다. 경로·쓰기 권한·설정 파일을 확인하세요. 기존 파일은 임의로 덮어쓰지 않습니다.';
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
    async browse(pick: () => Promise<string | null>) {
      if (state.busy) return;
      state.busy = true;
      state.error = '';
      try { const directory = await pick(); if (directory !== null) state.draft.downloadDirectory = directory; }
      catch { state.error = '폴더를 선택하거나 쓸 수 없습니다. 다른 저장 위치를 선택하세요.'; }
      finally { state.busy = false; }
    },
    async save(persist: (draft: Settings) => Promise<Settings>) {
      if (state.busy) return;
      state.busy = true;
      state.error = '';
      try {
        const saved = await persist({ ...state.draft });
        state.current = { ...saved };
        state.draft = { ...saved };
        state.open = false;
      } catch (error) { state.error = settingsError(error); }
      finally { state.busy = false; }
    },
  };
}
