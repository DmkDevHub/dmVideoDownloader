export interface Format {
  id: string;
  container: string | null;
  height: number | null;
}
export interface AnalysisReport {
  title: string;
  formats: Format[];
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
    case 'METADATA_LIMIT': return '영상 정보가 분석 한도를 초과했습니다.';
    default: return '이 주소의 영상 정보를 확인하지 못했습니다.';
  }
}
