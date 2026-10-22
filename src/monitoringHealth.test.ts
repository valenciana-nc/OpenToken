import { describe, expect, it } from 'vitest';
import { monitoringHealth } from './monitoringHealth';
import type { AccountState } from './types';

function state(kind: string, cached = true): AccountState {
  return { account: { id: 'sample', provider: 'copilot', label: 'Sample', email: null, sources: [] }, status: cached ? 'stale' : 'faulted', fault: { kind, message: 'Synthetic failure', retryAfterSecs: null }, report: cached ? { plan: null, observedAt: '2026-10-20T00:00:00Z', quotas: [] } : null };
}
describe('saved monitoring versus live reading health', () => {
  it.each(['offline', 'rate_limited', 'unreadable', 'unknown'])('%s is a read problem, not an invitation to reconnect', kind => {
    const h = monitoringHealth([state(kind)], true);
    expect(h.setup).toBe(false); expect(h.label).toContain('cached usage'); expect(h.message).toContain('not live');
  });
  it('does not invent a cache on cold-start failure', () => {
    const h = monitoringHealth([state('offline', false)], true);
    expect(h.setup).toBe(false); expect(h.label).toContain('usage unavailable'); expect(h.message).toContain('automatically');
  });
  it.each(['needs_relogin', 'not_signed_in', 'identity_changed', 'permission_denied', 'tool_missing'])('%s keeps monitoring saved while exposing explicit setup', kind => {
    const h = monitoringHealth([state(kind, false)], true);
    expect(h.setup).toBe(true); expect(h.label).toContain('Monitoring saved');
  });
  it('distinguishes unsaved, waiting, and ready providers', () => {
    expect(monitoringHealth([], false)).toMatchObject({ setup: true, label: null });
    expect(monitoringHealth([], true)).toMatchObject({ setup: false, label: 'Monitoring saved · usage unavailable' });
    const ready = state('offline'); ready.status = 'ready'; ready.fault = null;
    expect(monitoringHealth([ready], true)).toMatchObject({ setup: false, label: null });
  });
});
