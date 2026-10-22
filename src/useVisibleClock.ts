import { useEffect, useState } from 'react';
import { command, subscribe } from './bridge';

/** Native visibility events matter: a hidden WebView need not set document.hidden. */
export default function useVisibleClock() {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    let alive = true;
    let nativeVisible = !document.hidden;
    let timer: number | undefined;
    const schedule = () => {
      clearInterval(timer);
      if (alive && nativeVisible && !document.hidden) {
        setNow(Date.now());
        timer = window.setInterval(() => setNow(Date.now()), 5_000);
      }
    };
    const visible = (next: boolean) => { nativeVisible = next; schedule(); };
    const subscription = subscribe<boolean>('dashboard-visibility', visible);
    void command<{ visible?: boolean }>('get_desktop_status').then(s => { if (alive && s.visible != null) visible(s.visible); }).catch(() => {});
    document.addEventListener('visibilitychange', schedule); schedule();
    return () => { alive = false; clearInterval(timer); document.removeEventListener('visibilitychange', schedule); void subscription.then(stop => stop()).catch(() => {}); };
  }, []);
  return now;
}
