import { useEffect, useState } from 'react';
import { Info, ShieldCheck, X } from 'lucide-react';
import type { Connection, Settings, Snapshot } from './types';
import type { Page } from './ui';
import { defaultSettings, emptySnapshot } from './types';
import { age } from './format';
import { command, demo, hide, subscribe } from './bridge';
import PanelHeader from './components/StatusBar';
import ProviderGrid from './components/ProviderGrid';
import ProviderPanel from './components/ProviderPanel';
import { type ProviderId } from './providers';
import useSpending from './useSpending';
import useVisibleClock from './useVisibleClock';
import { trackingStatus } from './tracking';
import PreferencesPage from './pages/PreferencesPage';
import UsagePage, { type UsageFilter } from './pages/UsagePage';
import ProviderConnection from './components/ProviderConnection';

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>(emptySnapshot);
  const [settings, setSettings] = useState<Settings>(defaultSettings);
  const [draft, setDraft] = useState<Settings>(defaultSettings);
  const [page, setPage] = useState<Page>('usage');
  const [selectedProvider, setSelectedProvider] = useState<ProviderId>('codex');
  const [usageProvider, setUsageProvider] = useState<UsageFilter>('all');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const [settingsReady, setSettingsReady] = useState(false);
  const [pinning, setPinning] = useState(false);
  const now = useVisibleClock();
  const [reordering, setReordering] = useState(false);
  const spending = useSpending(settings.liveTracking);
  const live = trackingStatus(spending.tracking, now, demo);
  useEffect(() => {
    let alive = true;
    const update = (s: Snapshot) => { if (alive) setSnapshot(prev => s.revision >= prev.revision ? s : prev); };
    const unlisten = subscribe<Snapshot>('state-changed', update);
    const unnav = subscribe<Page>('navigate', p => { if (alive) setPage(p); });
    const unconnections = subscribe<Connection[]>('connections-changed', connections => {
      if (!alive) return;
      setSettings(cfg => ({ ...cfg, connections })); setDraft(cfg => ({ ...cfg, connections }));
      void command<Snapshot>('get_state').then(update).catch(() => {});
    });
    const uncli = subscribe<{ provider: 'codex' | 'claude'; settings: Settings }>('cli-accounts-changed', incoming => {
      if (!alive) return;
      const merge = (cfg: Settings) => ({ ...cfg, ...(incoming.provider === 'codex' ? { codexHomes: incoming.settings.codexHomes, codexEnabled: incoming.settings.codexEnabled } : { claudeHomes: incoming.settings.claudeHomes, claudeEnabled: incoming.settings.claudeEnabled }), ignoredHomes: incoming.settings.ignoredHomes });
      setSettings(merge); setDraft(merge);
      void command<Snapshot>('get_state').then(update).catch(() => {});
    });
    // First reveal waits for mounted content, so the native fade never opens an empty glass box.
    if (!demo) void command('dashboard_ready').catch(() => {});
    Promise.all([command<Snapshot>('get_state'), command<Settings>('get_settings')]).then(([s, cfg]) => {
      if (alive) { update(s); setSettings(cfg); setDraft(cfg); setSettingsReady(true); if (!cfg.codexEnabled && cfg.claudeEnabled) setSelectedProvider('claude'); else if (!cfg.codexEnabled && !cfg.claudeEnabled && cfg.connections.length) setSelectedProvider(cfg.connections[0].provider); }
    }).catch(e => { if (alive) setError(String(e)); });
    return () => { alive = false; void unlisten.then(f => f()).catch(() => {}); void unnav.then(f => f()).catch(() => {}); void unconnections.then(f => f()).catch(() => {}); void uncli.then(f => f()).catch(() => {}); };
  }, []);
  useEffect(() => {
    const system = matchMedia('(prefers-color-scheme: dark)');
    const apply = () => { document.documentElement.dataset.theme = settings.theme === 'system' ? system.matches ? 'dark' : 'light' : settings.theme; };
    apply(); system.addEventListener('change', apply); return () => system.removeEventListener('change', apply);
  }, [settings.theme]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape' && !e.defaultPrevented) void hide(); };
    addEventListener('keydown', key); return () => removeEventListener('keydown', key);
  }, []);
  async function action(run: () => Promise<unknown>, success = '') {
    setBusy(true); setError(''); setNotice('');
    try { await run(); setNotice(success); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function save(next = draft) {
    const saved = await command<Settings>('save_settings', { settings: next }); setSettings(saved); setDraft(saved);
  }
  async function connectionsChanged(_next: Settings) {
    const next=await command<Settings>('get_settings');
    const merge=(cfg:Settings)=>({...cfg,connections:next.connections,codexHomes:next.codexHomes,codexEnabled:next.codexEnabled,claudeHomes:next.claudeHomes,claudeEnabled:next.claudeEnabled,historySources:next.historySources,ignoredHomes:next.ignoredHomes});
    setSettings(merge);
    setDraft(merge);
    const current = await command<Snapshot>('get_state');
    setSnapshot(prev => current.revision >= prev.revision ? current : prev);
  }
  async function reorder(order: string[]) {
    const previous = settings.accountOrder;
    const next = [...order, ...previous.filter(id => !order.includes(id))];
    setReordering(true); setError('');
    setSettings(cfg => ({ ...cfg, accountOrder: next }));
    setDraft(cfg => ({ ...cfg, accountOrder: next }));
    try { await command('save_account_order', { order: next }); }
    catch (e) {
      setSettings(cfg => ({ ...cfg, accountOrder: previous }));
      setDraft(cfg => ({ ...cfg, accountOrder: previous }));
      setError(`Account order could not be saved. ${String(e)}`);
    } finally { setReordering(false); }
  }
  async function togglePin() {
    if (pinning || busy || !settingsReady) return;
    const previous = settings.dashboardPinned;
    setPinning(true); setError('');
    setSettings(cfg => ({ ...cfg, dashboardPinned: !previous }));
    setDraft(cfg => ({ ...cfg, dashboardPinned: !previous }));
    try { await command('set_dashboard_pinned', { pinned: !previous }); }
    catch (e) {
      setSettings(cfg => ({ ...cfg, dashboardPinned: previous }));
      setDraft(cfg => ({ ...cfg, dashboardPinned: previous }));
      setError(`Dashboard pin could not be saved. ${String(e)}`);
    } finally { setPinning(false); }
  }
  function navigate(next: Page) { setPage(next); setError(''); setNotice(''); document.querySelector('main')?.scrollTo({ top: 0 }); }
  async function copyReport() {
    // Never copy credential folders, tokens or opaque credential-store details.
    const report = { app: 'OpenToken', sampleData: demo, observedAt: snapshot.scannedAt, spending: spending.data, billingNote: 'Token values are estimates, not subscription charges. Credit changes are observed balance changes.', accounts: snapshot.accounts.map(({ account, report, status }) => ({ provider: account.provider, label: account.label, email: account.email, status, report })) };
    await navigator.clipboard.writeText(JSON.stringify(report, null, 2));
  }
  const latest = snapshot.accounts.flatMap(a => a.report ? [a.report.observedAt] : []).sort().at(-1) ?? null;
  const refreshing = snapshot.refreshing || busy;
  const preferences = { settings, draft, setDraft, save, action, connectionsChanged, busy: busy || pinning };
  return <div className="app">
    <div className="surface">
      <PanelHeader page={page} navigate={navigate} refreshing={refreshing} refresh={() => void action(() => command('refresh'))} pinned={settings.dashboardPinned} pinDisabled={pinning || busy || !settingsReady} togglePin={() => void togglePin()}/>
      {demo && <div className="demo-banner">Design preview · sample data, not your actual usage</div>}
      {page === 'usage' && <ProviderGrid selected={selectedProvider} select={id => { setSelectedProvider(id); document.querySelector('main')?.scrollTo({ top: 0 }); }} accounts={snapshot.accounts} add={() => navigate('accounts')}/>}
      <main>
        {error && <div className="alert error" role="alert"><Info size={17}/><span>{error}</span><button aria-label="Dismiss error" onClick={() => setError('')}><X size={15}/></button></div>}
        {snapshot.apiError && <div className="alert error" role="alert"><Info size={17}/>{snapshot.apiError}</div>}
        {notice && <div className="alert success" role="status">{notice}</div>}
        {page === 'usage' && <ProviderPanel key={selectedProvider} selected={selectedProvider} snapshot={snapshot} settings={settings} now={now} example={demo} savingOrder={reordering} reorder={reorder} manage={() => navigate('accounts')} connection={<ProviderConnection key={selectedProvider} provider={selectedProvider} busy={busy} action={action} changed={connectionsChanged}/>}/>}
        {page === 'spending' && <UsagePage provider={usageProvider} select={setUsageProvider} accounts={snapshot.accounts} example={demo} history={spending.data} historyError={spending.error} pricing={() => { navigate('settings'); requestAnimationFrame(() => document.querySelector('.model-rates')?.scrollIntoView({ block: 'start' })); }} copy={() => void action(copyReport, 'Usage report copied. No credentials or folder paths were included.')}/>}
        {(page === 'accounts' || page === 'settings') && <PreferencesPage {...preferences} section={page} navigate={navigate} snapshot={snapshot} refreshing={refreshing} provider={selectedProvider} pricing={spending.data}/>}
      </main>
      <footer><span className={`live-status ${live.active ? 'active' : ''}`} title={live.title}><ShieldCheck size={13}/><i aria-hidden="true"/>{live.label}</span><span>{age(page === 'spending' ? spending.data?.updatedAt ?? null : latest, now)}</span><button title="Open diagnostic log" onClick={() => void action(() => command('open_log'))}>OpenToken</button></footer>
    </div>
  </div>;
}
