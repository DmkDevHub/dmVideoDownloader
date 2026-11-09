import type { Candidate } from './media';
import type { NativeResult } from './native';
export interface PopupState {
  ready: boolean; tabId?: number; candidates: Candidate[]; autoDetect: boolean;
  settingsSource: 'default' | 'last-confirmed'; cacheAvailable: boolean;
  lastVerifiedAt?: string;
}
export function candidateLabel(candidate: Candidate): { title: string; site: string; type: string } {
  const title = (candidate.pageTitle || '영상 후보').replace(/https?:\/\/[^\s]+/giu, value => {
    try { return new URL(value).hostname; } catch { return '영상 주소'; }
  });
  return { title, site: new URL(candidate.pageUrl).hostname,
    type: { mp4: 'MP4', webm: 'WebM', hls: 'HLS', dash: 'DASH', unknown: '유형 미확인' }[candidate.sourceType] };
}
export function renderCandidates(doc: Document, state: PopupState, select: (candidate: Candidate) => Promise<NativeResult>): void {
  doc.querySelector('#count')!.textContent = `${state.candidates.length}개`;
  doc.querySelector('#detection')!.textContent = `자동 감지 ${state.autoDetect ? '켜짐' : '꺼짐'} · ${state.settingsSource === 'default' ? '기본값 사용 · 연결 테스트로 앱 설정 읽기' : '마지막으로 확인한 앱 설정 사용'}`;
  doc.querySelector('#cache')!.textContent = state.cacheAvailable ? '' : '세션 저장을 확인할 수 없습니다. 워커 재시작 시 후보가 사라질 수 있습니다.';
  doc.querySelector('#last-success')!.textContent = state.lastVerifiedAt ? `마지막 성공: ${new Date(state.lastVerifiedAt).toLocaleString()} · 현재 연결은 확인 필요` : '마지막 성공 기록 없음 · 현재 연결 확인 필요';
  doc.querySelector('#empty')!.textContent = state.candidates.length ? '' : '후보가 없습니다. 영상을 재생하고 Chrome의 사이트 접근을 확인하세요. blob·보호 영상은 원본 주소 확인이 안 될 수 있습니다.';
  const list = doc.querySelector('#candidates')!;
  list.replaceChildren();
  for (const candidate of state.candidates) {
    const row = doc.createElement('li');
    const label = candidateLabel(candidate);
    const title = doc.createElement('strong'); title.textContent = label.title;
    const detail = doc.createElement('p'); detail.textContent = `${label.site} · ${label.type} · 화질 미확인 · 보호 여부 미확인`;
    const button = doc.createElement('button'); button.textContent = '다운로드';
    button.addEventListener('click', () => {
      button.disabled = true;
      void select(candidate).then(result => { doc.querySelector('#status')!.textContent = result.safeMessage; })
        .catch(() => { doc.querySelector('#status')!.textContent = '후보를 전달하지 못했습니다. 앱 설정에서 연결을 확인하세요.'; })
        .finally(() => { button.disabled = false; });
    });
    row.append(title, detail, button); list.append(row);
  }
}
export async function copyExtensionId(id: string, write: (text: string) => Promise<void>): Promise<boolean> {
  if (!/^[a-p]{32}$/u.test(id)) return false;
  try { await write(id); return true; } catch { return false; }
}
