import type { AccountState } from './types';

/** Saved monitoring is not the same thing as a successful current quota read. */
export function monitoringHealth(accounts: AccountState[], saved: boolean) {
  const recovery = accounts.some(a => a.fault && ['needs_relogin', 'not_signed_in', 'identity_changed', 'tool_missing', 'permission_denied'].includes(a.fault.kind));
  const waiting = !recovery && (accounts.some(a => a.status !== 'ready') || (saved && !accounts.length));
  const cached = accounts.some(a => a.status === 'stale' && a.report);
  const rejected = accounts.some(a => a.fault && ['needs_relogin', 'not_signed_in'].includes(a.fault.kind));
  return {
    setup: recovery || (!saved && !accounts.length),
    label: rejected ? 'Monitoring saved · sign-in needs recovery' : recovery ? 'Monitoring saved · access needs attention' : waiting ? cached ? 'Monitoring saved · cached usage' : 'Monitoring saved · usage unavailable' : null,
    message: rejected ? 'The provider rejected or could not find the saved login. This connection has not been removed. Recovery is explicit; OpenToken does not renew the login itself.'
      : recovery ? 'The connection is still saved. Check the source error below; no different account or login was substituted.'
      : waiting ? cached ? 'Temporary read problem. Showing the last successful reading, not live usage. OpenToken retries automatically and respects provider pauses.' : 'No current reading is available. OpenToken retries automatically; you do not need to reconnect for a temporary read problem.' : null,
  };
}
