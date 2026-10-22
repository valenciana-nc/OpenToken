import { useEffect, useState } from 'react';
import { command, subscribe } from './bridge';
import type { TrackingState } from './types';
import { acceptTracking, emptyTracking } from './tracking';

export default function useSpending(live: boolean) {
  const [tracking, setTracking] = useState<TrackingState>(emptyTracking);
  const [connectionError, setConnectionError] = useState('');
  useEffect(() => {
    let alive = true;
    let connected = false;
    let timer: number | undefined;
    let unsubscribe: (() => void) | undefined;
    const update = (next: TrackingState) => { if (alive) { setTracking(previous => acceptTracking(previous, next)); if (connected) setConnectionError(''); } };
    async function sync() {
      // Subscribe before reading the snapshot; revisions close the event/IPC response race.
      if (!connected) {
        try {
          const stop = await subscribe<TrackingState>('spending-changed', update);
          if (!alive) { stop(); return; }
          unsubscribe = stop; connected = true;
        } catch (e) { if (alive) setConnectionError(`Live updates unavailable. ${String(e)}`); }
      }
      if (!alive) return;
      try { update(await command<TrackingState>('get_tracking')); }
      catch (e) { if (alive) setConnectionError(String(e)); }
      // Safety fallback only: reads the Rust cache, never scans files or calls a provider.
      if (alive) timer = window.setTimeout(() => void sync(), connected ? 120_000 : 15_000);
    }
    void sync();
    return () => { alive = false; clearTimeout(timer); unsubscribe?.(); };
  }, [live]);
  return { data: tracking.data, error: tracking.error ?? connectionError, tracking };
}
