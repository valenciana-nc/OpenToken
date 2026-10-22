import type { CursorLoginStatus } from './types';
export const idleCursorLogin: CursorLoginStatus = { revision: 0, phase: 'idle', message: '', failure: null };
export function acceptCursorLogin(current: CursorLoginStatus, incoming: CursorLoginStatus) { return incoming.revision >= current.revision ? incoming : current; }
export function cursorLoginActive(status: CursorLoginStatus) { return ['checking', 'waiting', 'connecting'].includes(status.phase); }
export function cursorLoginCancellable(status: CursorLoginStatus) { return status.phase === 'checking' || status.phase === 'waiting'; }
