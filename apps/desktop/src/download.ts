
import { analysisError } from './analysis';
import { settingsError } from './settings';

export type DownloadRequest = { kind: 'direct'; address: string } | { kind: 'candidate'; id: string };
export interface DownloadProgress { downloaded: number; total: number | null; bytesPerSecond: number | null; merging: boolean }
export interface SavedVideo { filename: string; height: number | null }

export function progressLabel(progress?: DownloadProgress) {
  if (!progress || progress.merging) return '—';
  const size = progress.total
    ? Math.floor(progress.downloaded / progress.total * 100) + '%'
    : (progress.downloaded / 1048576).toFixed(1) + ' MiB';
  return progress.bytesPerSecond == null ? size : size + ' · ' + (progress.bytesPerSecond / 1048576).toFixed(1) + ' MiB/s';
}

export function downloadError(error: unknown) {
  switch (error) {
    case 'DOWNLOAD_BUSY': return '다른 영상을 다운로드하고 있습니다.';
    case 'DOWNLOAD_FORMAT_UNSUPPORTED': return '저장할 수 있는 영상 포맷을 찾지 못했습니다.';
    case 'CONTAINER_INCOMPATIBLE': return '선택한 파일 형식에 영상과 음성을 담을 수 없습니다. 설정의 파일 형식을 확인하세요.';
    case 'FFMPEG_MISSING': return '실행 파일 옆 tools 폴더에 ffmpeg.exe를 넣고 다시 시도하세요.';
    case 'FFMPEG_UNSUPPORTED': return '지원되는 FFmpeg 버전으로 교체한 뒤 다시 시도하세요.';
    case 'FFMPEG_FAILED': return 'FFmpeg를 실행하지 못했습니다. 설정에서 도구 상태를 확인하세요.';
    case 'QUALITY_UNAVAILABLE': return '선택한 화질 상한 이내로 확인된 포맷이 없습니다. 설정의 화질을 확인하세요.';
    case 'DOWNLOAD_TIMEOUT': return '다운로드 시간이 초과됐습니다.';
    case 'DOWNLOAD_EXISTS': return '같은 이름의 파일이 이미 있습니다.';
    case 'DOWNLOAD_SAVE_FAILED': return '영상 파일을 저장하지 못했습니다.';
    case 'DOWNLOAD_FAILED': return '영상 다운로드에 실패했습니다.';
    case 'INVALID_SETTINGS':
    case 'UNSUPPORTED_SETTINGS_VERSION':
    case 'SETTINGS_BUSY':
    case 'INVALID_DIRECTORY':
    case 'ABSOLUTE_DIRECTORY_REQUIRED':
    case 'UNSAFE_DIRECTORY':
    case 'DIRECTORY_NOT_WRITABLE':
    case 'NOT_A_DIRECTORY': return settingsError(error);
    default: return analysisError(error);
  }
}
