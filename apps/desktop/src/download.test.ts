import { describe, expect, it } from 'vitest';
import { progressLabel, type DownloadProgress } from './download';

const progress: DownloadProgress = {
  downloaded: 500, total: 1000, bytesPerSecond: 100, segments: null, continuation: null,
};

describe('download progress', () => {
  it('shows the estimated time and verified continuation state', () => {
    expect(progressLabel({ ...progress, continuation: 'resumed' })).toContain('약 5초 남음 · 이어받는 중');
    expect(progressLabel({ ...progress, downloaded: 0, continuation: 'restarted' })).toContain('처음부터 다시 받는 중');
  });

  it('omits the estimate when total or speed is unknown', () => {
    expect(progressLabel({ ...progress, total: null })).not.toContain('남음');
    expect(progressLabel({ ...progress, bytesPerSecond: 0 })).not.toContain('남음');
  });
});
