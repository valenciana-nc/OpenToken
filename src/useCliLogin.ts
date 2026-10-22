import { useEffect, useState } from 'react';
import { command, subscribe } from './bridge';
import { acceptCursorLogin, idleCursorLogin } from './cursorLogin';
import type { CursorLoginStatus } from './types';
export type CliProvider = 'codex' | 'claude';
export default function useCliLogin(provider: CliProvider) {
  const [status, setStatus] = useState<CursorLoginStatus>(idleCursorLogin);
  useEffect(() => {
    let alive = true;
    const update = (incoming: CursorLoginStatus) => { if (alive) setStatus(current => acceptCursorLogin(current, incoming)); };
    const listener = subscribe<{ provider: CliProvider; status: CursorLoginStatus }>('cli-login-changed', incoming => { if (incoming.provider === provider) update(incoming.status); });
    void command<CursorLoginStatus>('get_cli_login_status', { provider }).then(update).catch(() => {});
    return () => { alive = false; void listener.then(remove => remove()).catch(() => {}); };
  }, [provider]);
  async function request(name: 'start_cli_sign_in' | 'cancel_cli_sign_in') {
    try {
      const next = await command<CursorLoginStatus>(name, { provider, ...(name === 'start_cli_sign_in' ? { consent: true } : {}) });
      setStatus(current => acceptCursorLogin(current, next));
    } catch (error) { setStatus(current => ({ ...current, phase: 'error', message: String(error), failure: 'unknown' })); }
  }
  return { status, start: () => request('start_cli_sign_in'), cancel: () => request('cancel_cli_sign_in') };
}
