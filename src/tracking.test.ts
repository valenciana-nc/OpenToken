import { describe, expect, it } from 'vitest';
import { acceptTracking, emptyTracking, trackingStatus } from './tracking';
import { age, countdown } from './format';

describe('live tracking presentation', () => {
  it('rejects an older IPC snapshot after a newer pushed update', () => {
    const current = { ...emptyTracking, revision: 9 };
    expect(acceptTracking(current, { ...emptyTracking, revision: 8 })).toBe(current);
    expect(acceptTracking(current, { ...emptyTracking, revision: 10 }).revision).toBe(10);
  });
  it('distinguishes real activity, quiet watching, fallback, off and errors', () => {
    const now = Date.now(); const state = { ...emptyTracking, mode: 'watching' as const, lastActivityAt: { codex: new Date(now - 5000).toISOString() } };
    expect(trackingStatus(state, now).active).toBe(true);
    expect(trackingStatus(state, now + 60_000).active).toBe(false);
    expect(trackingStatus({ ...state, mode: 'fallback' }, now).label).toBe('Periodic tracking');
    expect(trackingStatus({ ...state, mode: 'off' }, now).active).toBe(false);
    expect(trackingStatus({ ...state, error: 'Unavailable' }, now).label).toBe('Tracker unavailable');
    expect(trackingStatus(state, now, true)).toEqual({ label: 'Sample data', active: false, title: 'Design preview only; no live CLI activity.' });
  });
  it('ticks actual observation ages and final-minute countdowns without changing quotas', () => {
    const now = Date.now();
    expect(age(new Date(now - 10_000).toISOString(), now)).toBe('Updated 10s ago');
    expect(age('invalid', now)).toBe('Waiting for first reading');
    expect(countdown(new Date(now + 25_000).toISOString(), now)).toBe('Resets in 25s');
    expect(countdown(new Date(now + 25_000).toISOString(), now + 5000)).toBe('Resets in 20s');
  });
});
