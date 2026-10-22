import { useRef } from 'react';
import { Plus } from 'lucide-react';
import type { AccountState } from '../types';
import { remaining } from '../format';
import { providers, isTrackedProvider, isHistoryProvider, type ProviderId } from '../providers';
import ProviderGlyph from './ProviderGlyph';
import { monitoringHealth } from '../monitoringHealth';

export function providerRemaining(accounts: AccountState[], id: ProviderId) {
  const connected = accounts.filter(s => s.account.provider === id);
  if (!connected.length || connected.some(s => s.status !== 'ready')) return null;
  const values = connected.flatMap(s => {
    const quotas = s.report?.quotas.filter(q => id !== 'codex' && id !== 'claude' ? q.percent != null : /^(session|five_hour)$/i.test(q.id)) ?? [];
    const values = quotas.flatMap(q => { const value = remaining(q.percent); return value == null ? [] : [value]; });
    const value = values.length ? Math.min(...values) : null;
    return value == null ? [] : [value];
  });
  return values.length === connected.length ? Math.min(...values) : null;
}
export default function ProviderGrid({ selected, select, accounts, add }: { selected: ProviderId; select: (id: ProviderId) => void; accounts: AccountState[]; add: () => void }) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  return <nav className="provider-grid" aria-label="Providers">
    <div role="tablist" aria-label="Choose provider" className="provider-tabs">
      {providers.map((p, i) => {
        const left = providerRemaining(accounts, p.id);
        const count = accounts.filter(s => s.account.provider === p.id).length;
        const dashboard = accounts.some(s => s.account.provider === p.id && s.report?.source === 'cursor_dashboard');
        const internalCopilot = accounts.some(s => s.account.provider === p.id && s.report?.source === 'copilot_internal');
        const grokCli = accounts.some(s=>s.account.provider===p.id && s.report?.source==='grok_cli');
        const provisional = accounts.some(s => s.account.provider === p.id && s.report?.source === 'cli_log');
        const verified = count > 0 && accounts.filter(s => s.account.provider === p.id).every(s => s.status === 'ready' && !!s.report);
        const status = count ? `${count} connected ${p.id==='antigravity'&&accounts.some(a=>a.account.provider==='antigravity'&&a.report?.source==='antigravity_cli_usage')?'profile scope':'account'}${count === 1 ? '' : 's'} · ${!verified ? monitoringHealth(accounts.filter(s => s.account.provider === p.id), true).label ?? 'Cached or unavailable limits' : provisional ? 'CLI log; awaiting confirmation' : dashboard ? 'Includes unofficial Cursor dashboard' : internalCopilot ? 'Unofficial Copilot entitlement API' : grokCli ? 'Grok CLI · private credit API' : p.id==='devin' ? accounts.some(a=>a.account.provider==='devin'&&a.report?.source==='devin_app')?'Devin app · unofficial quota RPC':'Devin CLI · unofficial quota RPC' : p.id==='antigravity' ? accounts.some(a=>a.account.provider==='antigravity'&&a.report?.source==='antigravity_cli_usage')?'Antigravity CLI · profile-only quota':accounts.some(a=>a.account.provider==='antigravity'&&a.report?.source==='antigravity_stored')?'Antigravity IDE · stored session':accounts.some(a=>a.account.provider==='antigravity'&&a.report?.source==='antigravity_cli')?'Antigravity CLI · daily Cloud Code quota':'Antigravity app · owned local quota' : p.id==='muse' ? accounts.some(a=>a.account.provider==='muse'&&a.report?.source==='muse_device')?'Muse · authorized key-issuing quota endpoint':'Muse CLI · last-observed subscription' : 'Provider-verified limits'}` : isTrackedProvider(p.id) ? 'Not connected' : isHistoryProvider(p.id) ? 'Local token history · account quota unavailable' : 'Connector not available yet';
        return <button key={p.id} ref={node => { buttons.current[i] = node; }} id={`provider-tab-${p.id}`} role="tab" aria-selected={p.id === selected} aria-controls="provider-content" tabIndex={p.id === selected ? 0 : -1} className="provider-tab" title={`${p.name} · ${status}${left == null ? '' : ` · Lowest ${p.id !== 'codex' && p.id !== 'claude' ? 'reported allowance/budget' : 'session'}: ${left}% left`}`} onClick={() => select(p.id)} onKeyDown={e => {
          const offset = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowDown' ? 5 : e.key === 'ArrowUp' ? -5 : 0;
          if (!offset && e.key !== 'Home' && e.key !== 'End') return;
          e.preventDefault();
          const next = e.key === 'Home' ? 0 : e.key === 'End' ? providers.length - 1 : Math.max(0, Math.min(providers.length - 1, i + offset));
          select(providers[next].id); buttons.current[next]?.focus();
        }}>
          <ProviderGlyph provider={p.id} size={31}/><span className="provider-tab-label">{p.name}</span>
          {left != null && <span className="provider-usage-stripe" aria-hidden="true"><i style={{ width: `${left}%`, backgroundColor: p.accent }}/></span>}
        </button>;
      })}
    </div>
    <button className="provider-tab provider-add" aria-label="Add provider" title="Add a provider or another account" onClick={add}><span className="add-provider-mark" aria-hidden="true"><Plus size={31} strokeWidth={2.5}/></span><span className="provider-tab-label">Add</span></button>
  </nav>;
}
