import { describe, expect, it, vi } from 'vitest';
import type { KeyboardCoordinateGetter } from '@dnd-kit/core';
import type { AccountState } from '../types';
import { accountKeyboardCoordinates, orderAccounts } from './AccountList';
function coordinates(code: string, over: string, collisionTop: number) {
  const event = { code, preventDefault: vi.fn() } as unknown as KeyboardEvent;
  const args = { currentCoordinates: { x: 22, y: collisionTop }, context: { active: { id: 'a' }, over: { id: over }, collisionRect: { top: collisionTop, height: 275 }, droppableRects: new Map([['a', { top: 430, height: 275 }], ['b', { top: 730, height: 275 }]]) } } as unknown as Parameters<KeyboardCoordinateGetter>[1];
  return accountKeyboardCoordinates(['a', 'b'])(event, args);
}
describe('account ordering', () => {
  it('uses stable IDs, appends new identities and does not mutate the snapshot', () => {
    const accounts: AccountState[] = ['a', 'b', 'c'].map(id => ({ account: { id, provider: 'codex', label: id, email: null, sources: [] }, status: 'ready', report: null, fault: null }));
    expect(orderAccounts(accounts, ['missing', 'b']).map(s => s.account.id)).toEqual(['b', 'a', 'c']);
    expect(accounts.map(s => s.account.id)).toEqual(['a', 'b', 'c']);
  });
  it('keyboard Down targets the next card even after cancellation leaves stale screen geometry', () => {
    expect(coordinates('ArrowDown', 'a', -200)).toEqual({ x: 22, y: 730 });
    expect(coordinates('ArrowUp', 'b', 730)).toEqual({ x: 22, y: 430 });
  });
  it('does not wrap past the list edges or handle non-direction keys', () => {
    expect(coordinates('ArrowUp', 'a', 430)).toBeUndefined();
    expect(coordinates('ArrowDown', 'b', 730)).toBeUndefined();
    expect(coordinates('Enter', 'a', 430)).toBeUndefined();
  });
});
