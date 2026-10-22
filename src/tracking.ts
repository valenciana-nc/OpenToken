import type { TrackingState } from './types';
export const emptyTracking: TrackingState = { revision: 0, mode: 'starting', data: null, error: null, lastActivityAt: {} };
export function acceptTracking(previous: TrackingState, next: TrackingState): TrackingState {
  return next.revision >= previous.revision ? next : previous;
}
export function trackingStatus(state: TrackingState, now: number, example = false) {
  if (example) return { label: 'Sample data', active: false, title: 'Design preview only; no live CLI activity.' };
  if (state.error) return { label: 'Tracker unavailable', active: false, title: state.error };
  const active = Object.values(state.lastActivityAt).some(at => now - Date.parse(at) >= 0 && now - Date.parse(at) < 60_000);
  const labels = { starting: 'Starting tracker', watching: active ? 'Tracking usage' : 'Live local tracking', fallback: 'Periodic tracking', off: 'Live tracking off' };
  return { label: labels[state.mode], active: state.mode === 'watching' && active, title: state.mode === 'watching' ? 'Watching local CLI logs. Tokens update after records are written. Account limits are confirmed separately, up to once a minute while active; provider backoff still applies.' : state.mode === 'fallback' ? 'File watching is unavailable. Local history is checked every 15 seconds; account limits remain provider-verified.' : state.mode === 'off' ? 'Local history is checked periodically. Quotas use the idle refresh interval.' : 'Initializing the local history worker.' };
}
