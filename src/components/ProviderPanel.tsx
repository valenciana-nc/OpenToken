import { useEffect, useRef, useState, type ReactNode } from 'react';
import { ChevronRight, Info, Plus } from 'lucide-react';
import type { Settings, Snapshot } from '../types';
import { age } from '../format';
import { connectionScope, isHistoryProvider, isTrackedProvider, providerDefinition, type ProviderId } from '../providers';
import AccountList from './AccountList';
import { monitoringHealth } from '../monitoringHealth';

export default function ProviderPanel({ selected, snapshot, settings, now, example, savingOrder, reorder, manage, connection }:  {
  selected: ProviderId; snapshot: Snapshot; settings: Settings; now: number; example: boolean; savingOrder: boolean;
  reorder: (ids: string[]) => Promise<void>; manage: () => void;
  /** Connection controls; a function receives `true` when the user is adding another account. */
  connection?: ReactNode | ((adding: boolean) => ReactNode);
}) {
  const definition = providerDefinition(selected);
  const [adding, setAdding] = useState(false);
  const addRef = useRef<HTMLDivElement>(null);
  // The sign-in opens below the account list; bring it into view so the click visibly does something.
  // Deferred one frame: a smooth scroll started in the same commit is cancelled by the layout change.
  useEffect(() => {
    if (!adding) return;
    const frame = requestAnimationFrame(() => addRef.current?.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' }));
    return () => cancelAnimationFrame(frame);
  }, [adding]);
  const connect = (add: boolean) => typeof connection === 'function' ? connection(add) : connection;
  const accounts = snapshot.accounts.filter(s => s.account.provider === selected);
  const health = monitoringHealth(accounts, settings.connections.some(c => c.provider === selected));
  const latest = accounts.flatMap(a => a.report ? [a.report.observedAt] : []).sort().at(-1) ?? null;
  const tracked = isTrackedProvider(selected);
  const personalCursor = selected === 'cursor' && accounts.some(a => a.report?.source === 'cursor_dashboard' || settings.connections.some(c => c.id === a.account.id && c.mode === 'cursor_app'));
  const enterpriseCursor = selected === 'cursor' && accounts.some(a => a.report?.source !== 'cursor_dashboard' && !settings.connections.some(c => c.id === a.account.id && c.mode === 'cursor_app'));
  const githubQuota = selected === 'copilot' && accounts.some(a => a.report?.source === 'copilot_internal' || settings.connections.some(c => c.id === a.account.id && c.mode === 'github_cli'));
  const grokQuota = selected==='grok' && accounts.some(a=>['grok_cli','grok_cli_renewal'].includes(a.report?.source??'')||settings.connections.some(c=>c.id===a.account.id&&['grok_cli','grok_cli_renewal'].includes(c.mode??'')));
  const cliProfile=selected==='antigravity'&&accounts.some(a=>a.report?.source==='antigravity_cli_usage'||settings.connections.some(c=>c.id===a.account.id&&c.mode==='antigravity_cli_usage'));
  const localLabel = selected==='devin'?(accounts.some(a=>a.report?.source==='devin_app')?'Devin app quota API · ':'Devin CLI quota API · '):selected==='antigravity'?(accounts.some(a=>a.report?.source==='antigravity_cli_usage'||settings.connections.some(c=>c.id===a.account.id&&c.mode==='antigravity_cli_usage'))?'CLI-profile report · identity not provided · ':accounts.some(a=>a.report?.source==='antigravity_stored')?'Antigravity IDE stored session · ':accounts.some(a=>a.report?.source==='antigravity_cli')?'Antigravity CLI quota · ':'Antigravity local RPC · '):selected==='muse'?accounts.some(a=>a.report?.source==='muse_device'||settings.connections.some(c=>c.id===a.account.id&&c.mode==='muse_device'))?'Meta quota · authorized key endpoint · ':'Muse last observation · ':'';
  const plan = accounts.length === 1 ? accounts[0].report?.plan : accounts.length > 1 ? `${accounts.length} ${cliProfile?'scopes':'accounts'}` : null;
  return <section id="provider-content" role="tabpanel" aria-labelledby={`provider-tab-${selected}`}>
    <header className="provider-summary">
      <h2 aria-label={definition.name}><button className="provider-heading-link" aria-label={`Manage ${definition.name} accounts`} onClick={manage}><span>{definition.name}</span><ChevronRight size={20}/></button></h2>
      <div><span title={cliProfile?'Official CLI-profile observation; human account identity is not verified.':'Log updates require explicit matching identity; account limits are otherwise provider-verified.'}>{personalCursor ? 'Unofficial dashboard · ' : githubQuota ? 'Unofficial Copilot quota · ' : grokQuota ? 'Private Grok credit API · ' : accounts.length && localLabel ? localLabel : accounts.some(a => a.report?.source === 'cli_log') ? 'CLI log · ' : ''}{accounts.length ? age(latest, now) : isHistoryProvider(selected)&&settings.historySources[selected] ? 'Local tracking · no account quota' : tracked ? 'Not connected' : isHistoryProvider(selected) ? 'Local usage · no account quota' : 'No connector yet'}</span>{plan && <span>{plan}</span>}</div>
    </header>
    {(tracked || isHistoryProvider(selected)) ? <>
      {selected!=='codex'&&selected!=='claude' && accounts.length > 0 && <details className="connection-context"><summary><Info size={12} aria-hidden="true"/>Connection details</summary><p className="connection-scope">{cliProfile?'Official agy /usage reports the current CLI profile, not a verified human account. Approved background calls may renew the CLI-owned login until disconnected. No local history is assigned and old readings are not restored after failures or restart.':connectionScope(selected, personalCursor, githubQuota, accounts.some(a=>a.report?.source==='muse_device'||settings.connections.some(c=>c.id===a.account.id&&c.mode==='muse_device')))}</p>{personalCursor && enterpriseCursor && <p className="connection-scope">{connectionScope('cursor')}</p>}</details>}
      {accounts.some(a => a.status === 'ready' && a.report?.quotas.length === 0) && <p className="muted-text">{selected === 'cursor' ? 'Cursor did not report a numeric individual allowance. No remaining percentage or reset was inferred.' : selected === 'copilot' && githubQuota ? 'Copilot did not report a metered allowance. Unlimited or missing quotas are not treated as 100% left.' : selected === 'copilot' ? 'No personal Copilot billing records were returned. This does not establish zero usage or a remaining allowance.' : 'No numeric quota was supplied. No remaining percentage or reset was inferred.'}</p>}
      {selected === 'grok' && accounts.some(a => a.status === 'ready' && a.report && !a.report.plan && !a.report.quotas.some(q => q.percent != null)) && <p className="muted-text">xAI reported no subscription plan or usage meter for {accounts.filter(a => a.status === 'ready' && a.report && !a.report.plan && !a.report.quotas.some(q => q.percent != null)).map(a => a.account.email ?? a.account.label).join(', ')}, only a prepaid balance. If your SuperGrok or X Premium plan is on a different account, sign in to the Grok CLI with that account.</p>}
      {health.label && <div className="monitoring-health" role="status" aria-label={`${definition.name} monitoring status`}><strong>{health.label}</strong><p>{health.message}</p></div>}
      <AccountList accounts={accounts} order={settings.accountOrder} now={now} example={example} saving={savingOrder} reorder={reorder}/>
      {!health.setup && accounts.length > 0 && connection != null && <div className="add-account" ref={addRef}>
        {adding ? <>{connect(true)}<button className="text-button" type="button" onClick={() => setAdding(false)}>Done adding accounts</button></>
          : <button className="add-account-button" type="button" onClick={() => setAdding(true)}><Plus size={15} aria-hidden="true"/>Add another {definition.name} account</button>}
      </div>}
      {health.setup && (connect(false) ?? <div className="provider-empty"><h3>{snapshot.refreshing ? 'Finding your sign-ins…' : `Connect ${definition.name}`}</h3><p>Choose the provider’s connection option to verify identity and available usage.</p><button className="text-button" onClick={manage}><Plus size={15}/>Add an account</button></div>)}
    </> : (connect(false) ?? <div className="provider-empty"><h3>{definition.name} isn’t connected</h3><p>OpenToken doesn’t have a live {definition.name} connector yet. Working connector types are listed in Providers & settings; no usage or cost data is shown for unsupported providers.</p><button className="text-button" onClick={manage}><Plus size={15}/>Manage available connections</button></div>)}
  </section>;
}
