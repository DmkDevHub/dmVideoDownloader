
import { analysisError } from './analysis';
import { settingsError } from './settings';

export type DownloadRequest = { kind: 'direct'; address: string } | { kind: 'candidate'; id: string };
export interface DownloadProgress {
  downloaded: number; total: number | null; bytesPerSecond: number | null;
  segments: [completed: number, total: number] | null;
}
export type DownloadAction = 'pause' | 'resume' | 'cancel' | 'retry';
export const downloadPhase = {
  queued: '대기 중', analyzing: '영상 확인 중', downloading: '다운로드 중', merging: '파일 처리 중',
  retrying: '재시도 대기 중', pausing: '일시정지 중', cancelling: '취소 중', paused: '일시정지',
  completed: '완료', failed: '실패', cancelled: '취소됨',
};
export interface DownloadJob {
  id: number; title: string; phase: keyof typeof downloadPhase; height: number | null;
  progress: DownloadProgress | null; filename: string | null; error: string | null; attempt: number; canRetry: boolean;
}

export function progressLabel(progress: DownloadProgress | null) {
  if (!progress) return '—';
  const size = progress.total
    ? Math.floor(progress.downloaded / progress.total * 100) + '%'
    : (progress.downloaded / 1048576).toFixed(1) + ' MiB';
  const parts = progress.segments ? ' · 조각 ' + progress.segments.join('/') : '';
  const speed = progress.bytesPerSecond == null ? '' : ' · ' + (progress.bytesPerSecond / 1048576).toFixed(1) + ' MiB/s';
  return size + parts + speed;
}

export function downloadError(error: unknown) {
  switch (error) {
    case 'DOWNLOAD_QUEUE_FULL': return '다운로드 목록이 가득 찼습니다.';
    case 'DOWNLOAD_JOB_MISSING': return '다운로드 작업을 찾을 수 없습니다.';
    case 'DOWNLOAD_ACTION_UNAVAILABLE': return '현재 상태에서 해당 작업을 할 수 없습니다.';
    case 'DOWNLOAD_FORMAT_UNSUPPORTED': return '저장할 수 있는 영상 포맷을 찾지 못했습니다.';
    case 'HLS_PLAYLIST_UNSUPPORTED': return '지원하지 않는 HLS 재생목록 구성입니다.';
    case 'HLS_LIVE_UNSUPPORTED': return '종료된 HLS 영상만 저장할 수 있습니다.';
    case 'HLS_PROTECTED': return '암호화된 HLS 영상은 저장할 수 없습니다.';
    case 'STREAM_RESPONSE_INVALID': return '영상 서버가 올바른 재생목록 또는 조각을 반환하지 않았습니다.';
    case 'CONTAINER_INCOMPATIBLE': return '선택한 파일 형식에 영상과 음성을 담을 수 없습니다. 설정의 파일 형식을 확인하세요.';
    case 'FFMPEG_MISSING': return '실행 파일 옆 tools 폴더에 ffmpeg.exe를 넣고 다시 시도하세요.';
    case 'FFMPEG_UNSUPPORTED': return '지원되는 FFmpeg 버전으로 교체한 뒤 다시 시도하세요.';
    case 'FFMPEG_FAILED': return 'FFmpeg를 실행하지 못했습니다. 설정에서 도구 상태를 확인하세요.';
    case 'QUALITY_UNAVAILABLE': return '선택한 화질 상한 이내로 확인된 포맷이 없습니다. 설정의 화질을 확인하세요.';
    case 'DOWNLOAD_TIMEOUT': return '다운로드 시간이 초과됐습니다.';
    case 'DOWNLOAD_LIMIT': return '영상 전송량 또는 요청 수가 다운로드 한도를 초과했습니다.';
    case 'DOWNLOAD_EXISTS': return '같은 이름의 파일이 이미 있습니다.';
    case 'DOWNLOAD_SAVE_FAILED': return '영상 파일을 저장하지 못했습니다.';
    case 'DOWNLOAD_FAILED': return '영상 다운로드에 실패했습니다.';
    case 'INVALID_SETTINGS':
    case 'UNSUPPORTED_SETTINGS_VERSION':
    case 'SETTINGS_BUSY':
    case 'SETTINGS_READ_FAILED':
    case 'SETTINGS_TOO_LARGE':
    case 'INVALID_DIRECTORY':
    case 'ABSOLUTE_DIRECTORY_REQUIRED':
    case 'UNSAFE_DIRECTORY':
    case 'DIRECTORY_NOT_WRITABLE':
    case 'NOT_A_DIRECTORY': return settingsError(error);
    default: return analysisError(error);
  }
}
