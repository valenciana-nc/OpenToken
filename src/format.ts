import type { Quota } from './types';
export function remaining(used: number | null): number | null {
  return used == null || !Number.isFinite(used) ? null : Math.round(Math.max(0, Math.min(100, 100 - used)));
}
export function countdown(iso: string | null, now = Date.now()): string | null {
  if (!iso) return null;
  const date = Date.parse(iso);
  if (!Number.isFinite(date)) return null;
  const seconds = Math.ceil((date - now) / 1000);
  if (seconds <= 0) return 'Reset due';
  if (seconds < 60) return `Resets in ${seconds}s`;
  const mins = Math.ceil(seconds / 60);
  if (mins < 60) return `Resets in ${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return mins % 60 === 0 ? `Resets in ${hours}h` : `Resets in ${hours}h ${mins % 60}m`;
  return `Resets in ${Math.floor(hours / 24)}d ${hours % 24}h`;
}
export function amount(q: Quota): string {
  if (q.value != null) {
    if (q.unit === 'USD') return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(q.value);
    if (q.unit === 'credits') return `${q.value.toLocaleString()} credits · $${(q.value * .04).toFixed(2)}`;
    return `${q.value.toLocaleString()} ${q.unit ?? ''}`.trim();
  }
  return q.percent == null ? '—' : `${Math.round(q.percent)}%`;
}
export function age(iso: string | null, now = Date.now()): string {
  if (!iso) return 'Waiting for first reading';
  const seconds = Math.max(0, Math.floor((now - Date.parse(iso)) / 1000));
  if (!Number.isFinite(seconds)) return 'Waiting for first reading';
  const min = Math.floor(seconds / 60);
  return min < 1 ? seconds < 5 ? 'Updated just now' : `Updated ${seconds}s ago` : min < 60 ? `Updated ${min}m ago` : `Updated ${Math.floor(min / 60)}h ago`;
}
export function tone(percent: number | null) { return percent != null && percent >= 90 ? 'danger' : percent != null && percent >= 75 ? 'warning' : 'healthy'; }
