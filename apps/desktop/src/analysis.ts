export interface Format {
  id: string;
  container: string | null;
  width: number | null;
  height: number | null;
  videoCodec: string | null;
  audioCodec: string | null;
  tracks: 'audioVideo' | 'videoOnly' | 'audioOnly' | 'unknown';
}
export interface AnalysisReport {
  title: string;
  formats: Format[];
  source: 'direct' | 'page' | 'media';
}
export const analysisSource = { direct: '입력한 주소', page: '후보의 페이지 주소', media: '감지한 미디어 주소' };
export const trackLabel = { audioVideo: '영상·음성', videoOnly: '영상 전용', audioOnly: '음성 전용', unknown: '구성 미확인' };
export function resolution(format: Format) {
  if (format.height && format.width) return `${format.width} × ${format.height}`;
  if (format.height) return `${format.height}p`;
  return '미확인';
}
export function analysisError(error: unknown): string {
  switch (error) {
    case 'URL_NOT_ALLOWED': return '인증 정보가 없는 공개 HTTP 또는 HTTPS 주소를 입력하세요. 내부 주소로 향하는 요청은 차단됩니다.';
    case 'YT_DLP_MISSING': return '실행 파일 옆 tools 폴더에 yt-dlp.exe를 넣고 다시 시도하세요.';
    case 'YT_DLP_UNSUPPORTED': return '지원되는 yt-dlp 버전으로 교체한 뒤 다시 시도하세요.';
    case 'YT_DLP_FAILED': return 'yt-dlp를 실행하지 못했습니다. 설정에서 도구 상태를 확인하세요.';
    case 'ANALYSIS_BUSY': return '다른 주소를 분석하고 있습니다.';
    case 'ANALYSIS_TIMEOUT': return '분석 응답 시간이 초과됐습니다.';
    case 'NO_FORMATS': return '확인할 수 있는 영상 포맷이 없습니다.';
    case 'UNSUPPORTED_URL': return '이 페이지에서 지원되는 공개 영상을 찾지 못했습니다.';
    case 'CANDIDATE_MISSING': return '후보가 목록에 없습니다. Chrome에서 다시 선택하세요.';
    case 'METADATA_LIMIT': return '영상 정보가 분석 한도를 초과했습니다.';
    default: return '이 주소의 영상 정보를 확인하지 못했습니다.';
  }
}
