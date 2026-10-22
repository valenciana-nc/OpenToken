import { useEffect, useState } from 'react';
import type { CursorLoginStatus } from './types';
import { command, subscribe } from './bridge';
import { acceptCursorLogin, idleCursorLogin } from './cursorLogin';
export default function useGithubLogin(enabled: boolean, provider: 'github' | 'grok' | 'antigravity' = 'github') {
  const [status, setStatus] = useState<CursorLoginStatus>(idleCursorLogin);
  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    const update = (incoming: CursorLoginStatus) => { if (alive) setStatus(current => acceptCursorLogin(current, incoming)); };
    const listener = subscribe<CursorLoginStatus>(`${provider}-login-changed`, update);
    void command<CursorLoginStatus>(`get_${provider}_login_status`).then(update).catch(() => {});
    return () => { alive = false; void listener.then(remove => remove()).catch(() => {}); };
  }, [enabled, provider]);
  async function request(action: 'start' | 'cancel', extra?:Record<string,unknown>) {
    try { const incoming = await command<CursorLoginStatus>(`${action}_${provider}_sign_in`, action === 'start' ? { ...extra,consent: true } : undefined); setStatus(current => acceptCursorLogin(current, incoming)); }
    catch (error) { setStatus(current => ({ ...current, phase: 'error', message: String(error), failure: 'unknown' })); }
  }
  return { status, start: (extra?:Record<string,unknown>) => request('start',extra), recover: () => request('start',{recover:true}), cancel: () => request('cancel') };
}
