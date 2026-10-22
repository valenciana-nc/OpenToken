import { describe, expect, it } from 'vitest';
import { acceptCursorLogin, cursorLoginActive, cursorLoginCancellable, idleCursorLogin } from './cursorLogin';
import type { CursorLoginStatus } from './types';
describe('native Cursor sign-in state', () => {
  it('rejects stale cached/pushed states after a newer transition', () => {
    const connected: CursorLoginStatus = { revision: 8, phase: 'connected', message: 'Cursor connected.', failure: null };
    expect(acceptCursorLogin(connected, { ...idleCursorLogin, revision: 2 })).toBe(connected);
    expect(acceptCursorLogin(idleCursorLogin, connected)).toBe(connected);
  });
  it('allows cancellation only before the targeted verification transaction', () => {
    for (const phase of ['idle', 'checking', 'waiting', 'connecting', 'connected', 'cancelled', 'error'] as const) {
      const state = { ...idleCursorLogin, phase };
      expect(cursorLoginActive(state)).toBe(['checking', 'waiting', 'connecting'].includes(phase));
      expect(cursorLoginCancellable(state)).toBe(['checking', 'waiting'].includes(phase));
    }
  });
});
