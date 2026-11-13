import { useState } from 'react';
import { Check, RefreshCw, Trash2 } from 'lucide-react';
import type { Settings, Snapshot } from '../types';
import { providers, type ProviderId } from '../providers';
import type { PreferencesProps } from '../ui';
import { command } from '../bridge';
import ProviderGlyph from '../components/ProviderGlyph';
import ProviderConnection from '../components/ProviderConnection';
import SourceInventory from '../components/SourceInventory';

export default function AccountsPage({ snapshot, settings, draft, setDraft, save, action, busy, refreshing, connectionsChanged, initialProvider = 'codex' }: PreferencesProps & { snapshot: Snapshot; refreshing: boolean; initialProvider?: ProviderId }) {
  const [provider, setProvider] = useState<ProviderId>(initialProvider);
  return <div><p className="page-subtitle">Use existing sessions and recorded usage first. Account setup is only for missing access or another account.</p>
    <section className="panel"><h2>Monitored quota scopes <span className="count">{snapshot.accounts.length}</span></h2>
      {snapshot.accounts.map(({ account }) => <div className="identity-row" key={account.id}><ProviderGlyph provider={account.provider}/><div><b>{account.email ?? account.label}</b><small>{providers.find(p => p.id === account.provider)?.name} · {(account.provider==='codex'||account.provider==='claude') ? 'Existing CLI usage source' : settings.connections.some(c => c.id === account.id && c.mode === 'cursor_app') ? 'Cursor app · unofficial dashboard' : 'Provider API'}</small><input aria-label={`Name for ${account.label}`} placeholder="Account nickname" value={draft.aliases[account.id] ?? ''} onChange={e => setDraft({ ...draft, aliases: { ...draft.aliases, [account.id]: e.target.value } })}/></div>{(account.provider==='codex'||account.provider==='claude') && <button className="icon-button" aria-label={`Disconnect ${account.email ?? account.label}`} disabled={busy} onClick={() => void action(async () => { await command<Settings>('disconnect_cli_account', { id: account.id }); }, 'Monitoring stopped. Your CLI sign-in was not changed.')}><Trash2 size={16}/></button>}</div>)}
      {!snapshot.accounts.length && <p className="muted-text">No accounts yet. Choose a supported provider below.</p>}
      {!!snapshot.accounts.length && <button className="text-button" disabled={busy} onClick={() => void action(() => save({ ...settings, aliases: draft.aliases }), 'Account names saved.')}><Check size={15}/>Save names</button>}
    </section>
    {settings.connections.length > 0 && <section className="panel"><h2>Saved provider connections</h2><p className="hint">Disconnect stops monitoring without signing you out of the provider.</p>{settings.connections.map(c => <div className="identity-row" key={c.id}><ProviderGlyph provider={c.provider}/><div><b>{settings.aliases[c.id] || c.login}</b><small>{c.mode === 'devin_app' ? 'Devin app · unofficial quota · credential scope' : c.mode === 'devin_cli' ? 'Devin CLI · unofficial quota · credential scope' : c.mode === 'antigravity_cli_usage' ? 'Official agy /usage · current CLI profile · identity not provided' : c.mode === 'antigravity_stored' ? 'Antigravity IDE · stored session · daily Cloud Code quota' : c.mode === 'antigravity_cli' ? 'Antigravity CLI · daily Cloud Code quota' : c.mode === 'antigravity_app' ? 'Antigravity app · owned local quota' : c.mode === 'muse_device' ? 'Muse · authorized key-issuing quota endpoint' : c.mode === 'muse_cli' ? 'Muse CLI · last-observed subscription' : c.mode === 'grok_cli_renewal' ? 'Grok private credit API · official CLI renewal approved' : c.mode === 'grok_cli' ? 'Grok CLI · private credit API · no automatic renewal' : c.mode === 'github_cli' ? 'GitHub CLI · unofficial Copilot quota' : c.mode === 'cursor_app' ? 'Personal Cursor · unofficial dashboard' : c.provider === 'cursor' ? `Cursor Enterprise · selected member · connection ${c.id.slice(-6)}` : c.provider === 'copilot' ? 'Copilot · personal billing only' : `${providers.find(p => p.id === c.provider)?.name} · API-key scope · ${c.scope.split(':')[0]}`}</small></div><button className="icon-button" aria-label={`Disconnect ${c.login}`} disabled={busy} onClick={() => void action(async () => { const next = await command<Settings>('disconnect_provider', { id: c.id }); await connectionsChanged?.(next); }, 'Disconnected from OpenToken. Your provider login was not changed.')}><Trash2 size={16}/></button></div>)}</section>}
    <SourceInventory select={setProvider} busy={busy} action={action} changed={async next=>{await connectionsChanged?.(next);}}/>
    <section className="panel"><h2>Connect existing usage</h2>
      <label>Provider<select value={provider} disabled={busy} onChange={e => setProvider(e.target.value as ProviderId)}>{providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
      <ProviderConnection key={provider} provider={provider} busy={busy} action={action} accounts={snapshot.accounts} changed={async next => { await connectionsChanged?.(next); }}/>
    </section>
    <button className="text-button" disabled={refreshing} onClick={() => void action(() => command('refresh'), 'Source discovery requested. Existing monitoring was refreshed; no login was started.')}><RefreshCw size={15}/>Refresh source discovery</button>
  </div>;
}
