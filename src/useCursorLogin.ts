import { useEffect, useState } from 'react';
import type { CursorLoginStatus } from './types';
import { command, subscribe } from './bridge';
import { acceptCursorLogin, idleCursorLogin } from './cursorLogin';
export default function useCursorLogin(enabled: boolean) {
  const [status, setStatus] = useState<CursorLoginStatus>(idleCursorLogin);
  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    const update = (incoming: CursorLoginStatus) => { if (alive) setStatus(current => acceptCursorLogin(current, incoming)); };
    const listener = subscribe<CursorLoginStatus>('cursor-login-changed', update);
    void command<CursorLoginStatus>('get_cursor_login_status').then(update).catch(() => {});
    return () => { alive = false; void listener.then(remove => remove()).catch(() => {}); };
  }, [enabled]);
  async function request(name: 'start_cursor_sign_in' | 'cancel_cursor_sign_in') {
    try {
      const incoming = await command<CursorLoginStatus>(name, name === 'start_cursor_sign_in' ? { consent: true } : undefined);
      setStatus(current => acceptCursorLogin(current, incoming));
    } catch (error) {
      setStatus(current => ({ ...current, phase: 'error', message: String(error), failure: 'unknown' }));
    }
  }
  return { status, start: () => request('start_cursor_sign_in'), cancel: () => request('cancel_cursor_sign_in') };
}
