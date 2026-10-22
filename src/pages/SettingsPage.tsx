import { Activity, ArrowLeft, Check, LoaderCircle, LogOut } from 'lucide-react';
import type { Settings, Spending } from '../types';
import type { PreferencesProps } from '../ui';
import { command } from '../bridge';
import Toggle from '../components/Toggle';
import ModelRates from '../components/ModelRates';
import notices from '../../docs/THIRD-PARTY-LICENSES.txt?raw';
import license from '../../LICENSE?raw';

export default function SettingsPage({ settings, draft, setDraft, save, action, busy, pricing }: PreferencesProps & { pricing?: Spending | null }) {
  return <div><p className="page-subtitle">Appearance, discovery and background updates.</p>
    <section className="panel"><h2>Providers & discovery</h2>
      <Toggle label="Codex" description="Session, weekly, Spark and credits" checked={draft.codexEnabled} onChange={v => setDraft({ ...draft, codexEnabled: v })}/>
      <Toggle label="Claude" description="Session, weekly and model-specific limits" checked={draft.claudeEnabled} onChange={v => setDraft({ ...draft, claudeEnabled: v })}/>
      <Toggle label="Find existing Codex accounts" description="Discover known local sign-ins automatically" checked={draft.autoDiscover} onChange={v => setDraft({ ...draft, autoDiscover: v })}/>
    </section>
    <section className="panel"><h2>Appearance & refresh</h2>
      <div className="two-columns"><label>Theme<select value={draft.theme} onChange={e => setDraft({ ...draft, theme: e.target.value as Settings['theme'] })}><option value="light">Light</option><option value="dark">Dark</option><option value="system">System</option></select></label><label>Idle quota refresh<select value={draft.refreshMinutes} onChange={e => setDraft({ ...draft, refreshMinutes: +e.target.value })}>{[1, 5, 15, 30].map(n => <option value={n} key={n}>{n} minute{n > 1 ? 's' : ''}</option>)}</select></label></div>
      <Toggle label="Live tracking" description="Watch local CLI logs; refresh active account limits automatically" checked={draft.liveTracking} onChange={v => setDraft({ ...draft, liveTracking: v })}/>
      <p className="hint">Token totals update after the CLI writes usage. Active quotas are verified up to once a minute, then return to the idle interval. Anonymous log percentages never replace an account’s verified limits. Provider retry delays still apply.</p>
      <label>Tray icon style<select value={draft.trayStyle} onChange={e => setDraft({ ...draft, trayStyle: e.target.value as Settings['trayStyle'] })}><option value="badge">Fluent percentage · single icon</option><option value="provider_pair">Provider logo + stacked percentages</option></select></label>
      <label>Tray icon shows<select value={draft.trayMetric} onChange={e => setDraft({ ...draft, trayMetric: e.target.value as Settings['trayMetric'] })}><option value="auto">{draft.trayStyle === 'badge' ? 'Automatic · session first' : 'Session + weekly · stacked percentages'}</option><option value="session">Session remaining</option><option value="weekly">Weekly remaining</option></select></label>
      <label>Dashboard position<select value={draft.flyoutPosition} onChange={e => setDraft({ ...draft, flyoutPosition: e.target.value as Settings['flyoutPosition'] })}><option value="bottom_left">Bottom left</option><option value="bottom_right">Bottom right · system tray</option><option value="tray">Beside clicked tray icon</option></select></label>
      <button className="text-button" onClick={() => void action(() => command('open_tray_settings'))}><Activity size={16}/>Windows usage-icon visibility</button><p className="hint">The Fluent badge shows a clear percentage with a smooth remaining-quota ring. The optional logo pair shows session above weekly. Windows controls tray order and overflow.</p>
      <Toggle label="Launch with Windows" description="One Scheduled Task · recovery from next sign-in" checked={draft.launchAtStartup} onChange={v => setDraft({ ...draft, launchAtStartup: v })}/>
    </section>
    <section className="panel"><h2>Local API & credentials</h2><Toggle label="Read-only local API" description="Native clients only · browser origins rejected" checked={draft.apiEnabled} onChange={v => setDraft({ ...draft, apiEnabled: v })}/><label>Loopback port<input type="number" min={1024} max={65535} value={draft.apiPort} onChange={e => setDraft({ ...draft, apiPort: +e.target.value })}/></label><Toggle label="Renew Claude tokens" description="Locked, conflict-checked write-back to the CLI file" checked={draft.claudeTokenRefresh} onChange={v => setDraft({ ...draft, claudeTokenRefresh: v })}/><p className="hint">Claude Code may use a different lock. Disable renewal if you prefer it to own all token rotation. Codex always owns its own refresh tokens.</p></section>
    <ModelRates draft={draft} setDraft={setDraft} busy={busy} pricing={pricing}/>
    <section className="panel"><h2>Moving from QuotaTray?</h2><p className="muted-text">Import provider preferences and folders without touching the old app or its startup entries. Review, then save.</p><button className="text-button" onClick={() => void action(async () => { const imported = await command<Settings>('import_quotatray'); setDraft(imported); }, 'Imported into this form. Review and save when ready.')}><ArrowLeft size={15}/>Import QuotaTray settings</button></section>
    <div className="save-bar"><button className="secondary" disabled={busy} onClick={() => setDraft(settings)}>Discard</button><button className="primary" disabled={busy} onClick={() => void action(() => save(), 'Preferences saved.')} >{busy ? <LoaderCircle size={16} className="spin"/> : <Check size={16}/>}Save preferences</button></div>
    <details className="legal-notices"><summary>Open-source notices</summary><pre>{license}{'\n\n'}{notices}</pre></details>
    <button className="text-button danger-text" onClick={() => void action(() => command('quit'))}><LogOut size={15}/>Exit OpenToken</button>
  </div>;
}
