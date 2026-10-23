import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { defaultSettings, type AccountState, type Connection, type CursorLoginStatus, type Settings, type Snapshot } from './types';
document.documentElement.dataset.desktop = String(isTauri());
export const demo = new URLSearchParams(location.search).has('demo');
const now = Date.now();
const reset = (hours: number) => new Date(now + hours * 3_600_000).toISOString();
const fixture: Snapshot = { revision: 1, refreshing: false, scannedAt: new Date(now).toISOString(), folders: [], apiError: null, accounts: [
  { account: { id: 'demo-personal', provider: 'codex', label: 'Personal', email: 'alex@example.com', sources: [{ path: '~/.codex', store: 'file' }] }, status: 'ready', fault: null, report: { plan: 'Pro 200', observedAt: new Date(now).toISOString(), quotas: [
    { id: 'session', label: 'Session', percent: 24, value: null, unit: null, resetsAt: reset(3.2) }, { id: 'weekly', label: 'Weekly', percent: 67, value: null, unit: null, resetsAt: reset(75) }, { id: 'credits', label: 'Credits', percent: null, value: 821, unit: 'credits', resetsAt: null }, { id: 'resets', label: 'Rate limit resets', percent: null, value: 2, unit: 'available', resetsAt: null }] } },
  { account: { id: 'demo-work', provider: 'claude', label: 'Claude', email: null, sources: [{ path: '~/.claude', store: 'file' }] }, status: 'ready', fault: null, report: { plan: 'Team 5x', observedAt: new Date(now).toISOString(), quotas: [
    { id: 'five_hour', label: 'Session', percent: 0, value: null, unit: null, resetsAt: reset(5) }, { id: 'seven_day', label: 'Weekly', percent: 64, value: null, unit: null, resetsAt: reset(30), projectedRemainingAtReset: 22 }, { id: 'model-fable', label: 'Fable', percent: 73, value: null, unit: null, resetsAt: reset(30), projectedRemainingAtReset: 11 }] } }
] };
fixture.accounts.push({ account: { id: 'demo-codex-work', provider: 'codex', label: 'Work', email: 'dev@example.test', sources: [{ path: '~/.codex-work', store: 'file' }] }, status: 'ready', fault: null, report: { plan: 'Plus', observedAt: new Date(now).toISOString(), quotas: [
  { id: 'session', label: 'Session', percent: 47, value: null, unit: null, resetsAt: reset(2) },
  { id: 'weekly', label: 'Weekly', percent: 12, value: null, unit: null, resetsAt: reset(96) },
] } });
let demoSettings: Settings = { ...defaultSettings, claudeEnabled: true };
if (demo) { try { demoSettings = { ...demoSettings, ...JSON.parse(localStorage.getItem('opentoken.demo.settings') ?? '{}') }; } catch { /* Ignore an invalid preview-only preference. */ } }
let demoMuseHasRecords=demo&&localStorage.getItem('opentoken.demo.muse-records')==='yes';
const demoListeners = new Map<string, Set<(payload: unknown) => void>>();
function emitDemo(event: string, payload: unknown) { for (const listener of demoListeners.get(event) ?? []) listener(structuredClone(payload)); }
function saveDemo() { localStorage.setItem('opentoken.demo.settings', JSON.stringify(demoSettings)); emitDemo('connections-changed', demoSettings.connections); }
let demoCursorLogin: CursorLoginStatus = { revision: 0, phase: 'idle', message: '', failure: null };
let demoCursorRun = 0;
let demoGrokLogin:CursorLoginStatus={revision:0,phase:'idle',message:'',failure:null};let demoGrokRun=0;
function demoGrokPhase(phase:CursorLoginStatus['phase'],message:string,failure:string|null=null){demoGrokLogin={revision:demoGrokLogin.revision+1,phase,message,failure};emitDemo('grok-login-changed',demoGrokLogin);}
function finishDemoGrok(run:number){if(run!==demoGrokRun||!['checking','waiting'].includes(demoGrokLogin.phase))return;demoGrokPhase('connecting','Connecting Grok…');const old=demoSettings;try{const c:Connection={id:'demo-grok-cli',provider:'grok',subject:'sample-grok',login:'grok@example.test',scope:'grok:sample',mode:'grok_cli',profile:'C:\\Preview\\Grok'};demoSettings={...demoSettings,connections:[...demoSettings.connections.filter(c=>!['grok_cli','grok_cli_renewal'].includes(c.mode??'')),c]};saveDemo();demoGrokPhase('connected','Grok connected.');}catch(error){demoSettings=old;demoGrokPhase('error',String(error),'unknown');}}
if(demo)addEventListener('opentoken-demo-grok-ready',()=>finishDemoGrok(demoGrokRun));
let demoAntigravityLogin:CursorLoginStatus={revision:0,phase:'idle',message:'',failure:null};let demoAntigravityRun=0;
function demoAntigravityPhase(phase:CursorLoginStatus['phase'],message:string,failure:string|null=null){demoAntigravityLogin={revision:demoAntigravityLogin.revision+1,phase,message,failure};emitDemo('antigravity-login-changed',demoAntigravityLogin);}
function finishDemoAntigravity(run:number){if(run!==demoAntigravityRun||!['checking','waiting'].includes(demoAntigravityLogin.phase))return;const old=demoSettings;try{const c:Connection={id:'demo-antigravity-recovered',provider:'antigravity',subject:'sample-recovered',login:'antigravity@example.test',scope:'local:antigravity:sample-recovered',mode:'antigravity_app',profile:'C:\\Preview\\Antigravity IDE'};demoSettings={...demoSettings,connections:[...demoSettings.connections.filter(c=>c.provider!=='antigravity'),c]};saveDemo();demoAntigravityPhase('connected','Antigravity connected.');}catch(error){demoSettings=old;demoAntigravityPhase('error',String(error),'unknown');}}
if(demo)addEventListener('opentoken-demo-antigravity-ready',()=>finishDemoAntigravity(demoAntigravityRun));
let demoGithubLogin: CursorLoginStatus = { revision:0,phase:'idle',message:'',failure:null }; let demoGithubRun=0;
function demoGithubPhase(phase:CursorLoginStatus['phase'],message:string,failure:string|null=null){demoGithubLogin={revision:demoGithubLogin.revision+1,phase,message,failure};emitDemo('github-login-changed',demoGithubLogin);}
function finishDemoGithub(run:number){if(run!==demoGithubRun||!['checking','waiting'].includes(demoGithubLogin.phase))return;demoGithubPhase('connecting','Connecting Copilot…');const old=demoSettings;try{const c:Connection={id:'demo-copilot-cli',provider:'copilot',subject:'101',login:'github-sample',scope:'gh:sample',mode:'github_cli',profile:'C:\\Preview\\GitHub CLI'};demoSettings={...demoSettings,connections:[...demoSettings.connections.filter(c=>c.mode!=='github_cli'),c]};saveDemo();demoGithubPhase('connected','Copilot connected.');}catch(error){demoSettings=old;demoGithubPhase('error',String(error),'unknown');}}
if(demo)addEventListener('opentoken-demo-github-ready',()=>finishDemoGithub(demoGithubRun));
function demoLoginPhase(phase: CursorLoginStatus['phase'], message: string, failure: string | null = null) {
  demoCursorLogin = { revision: demoCursorLogin.revision + 1, phase, message, failure }; emitDemo('cursor-login-changed', demoCursorLogin);
}
async function finishDemoCursorLogin(run: number) {
  if (run !== demoCursorRun || !['checking', 'waiting'].includes(demoCursorLogin.phase)) return;
  demoLoginPhase('connecting', 'Connecting Cursor…');
  try { await command('connect_cursor_personal', { consent: true }); demoLoginPhase('connected', 'Cursor connected.'); }
  catch (error) { demoLoginPhase('error', String(error), 'unknown'); }
}
if (demo) addEventListener('opentoken-demo-cursor-ready', () => void finishDemoCursorLogin(demoCursorRun));
type CliProvider = 'codex' | 'claude';
const demoCliLogin: Record<CliProvider, CursorLoginStatus> = { codex: { revision: 0, phase: 'idle', message: '', failure: null }, claude: { revision: 0, phase: 'idle', message: '', failure: null } };
const demoCliRuns = { codex: 0, claude: 0 };
let demoCliAccounts: AccountState[] = [];
let demoExcluded: string[] = [];
if (demo) { try { demoCliAccounts = JSON.parse(localStorage.getItem('opentoken.demo.cli') ?? '[]'); demoExcluded = JSON.parse(localStorage.getItem('opentoken.demo.excluded') ?? '[]'); } catch { /* Synthetic data only. */ } }
function demoCliPhase(provider: CliProvider, phase: CursorLoginStatus['phase'], message: string, failure: string | null = null) {
  demoCliLogin[provider] = { revision: demoCliLogin[provider].revision + 1, phase, message, failure }; emitDemo('cli-login-changed', { provider, status: demoCliLogin[provider] });
}
async function finishDemoCli(provider: CliProvider, run: number) {
  if (demoCliRuns[provider] !== run || !['checking', 'waiting'].includes(demoCliLogin[provider].phase)) return;
  const name = provider === 'codex' ? 'Codex' : 'Claude';
  demoCliPhase(provider, 'connecting', `Connecting ${name}…`);
  const previous = demoSettings; const previousAccounts = demoCliAccounts;
  try {
    if (new URLSearchParams(location.search).get(`${provider}Login`) === 'failure') throw new Error('Sample usage verification failed. No account was connected.');
    const id = `demo-${provider}-extra-${demoCliAccounts.length + 1}`;
    const home = `C:\\Preview\\sign-ins\\${provider}\\${id}`;
    const source = fixture.accounts.find(a => a.account.provider === provider)!;
    const state: AccountState = { ...structuredClone(source), account: { ...source.account, id, email: provider === 'codex' ? `${id}@example.test` : null, label: `${name} · sample account`, sources: [{ path: home, store: 'file' }] } };
    demoCliAccounts = [...demoCliAccounts, state];
    demoSettings = { ...demoSettings, ...(provider === 'codex' ? { codexEnabled: true, codexHomes: [...demoSettings.codexHomes, home] } : { claudeEnabled: true, claudeHomes: [...demoSettings.claudeHomes, home] }) };
    localStorage.setItem('opentoken.demo.cli', JSON.stringify(demoCliAccounts));
    saveDemo(); emitDemo('cli-accounts-changed', { provider, settings: demoSettings });
    demoCliPhase(provider, 'connected', `${name} connected.`);
  } catch (error) {
    demoSettings = previous; demoCliAccounts = previousAccounts;
    try { if (previousAccounts.length) localStorage.setItem('opentoken.demo.cli', JSON.stringify(previousAccounts)); else localStorage.removeItem('opentoken.demo.cli'); } catch { /* Synthetic storage may be deliberately failing. */ }
    demoCliPhase(provider, 'error', String(error), 'unknown');
  }
}
if (demo) addEventListener('opentoken-demo-cli-ready', event => { const provider = (event as CustomEvent<CliProvider>).detail; if (provider === 'codex' || provider === 'claude') void finishDemoCli(provider, demoCliRuns[provider]); });
const demoExistingWaiters=new Map<string,{resolve:()=>void;reject:(error:Error)=>void}>();
if(demo)addEventListener('opentoken-demo-existing-ready',event=>{const provider=String((event as CustomEvent).detail);const waiter=demoExistingWaiters.get(provider);if(waiter){demoExistingWaiters.delete(provider);waiter.resolve();}});
const demoReadFailures = new Map<string, { kind: string; report: AccountState['report'] }>();
// Synthetic transport/status transitions only; production never consumes these events.
if (demo) addEventListener('opentoken-demo-source-fault', event => {
  const { provider, kind, cached = true } = (event as CustomEvent<{ provider: string; kind: string | null; cached?: boolean }>).detail;
  if (!['grok', 'copilot'].includes(provider)) return;
  const report = cached ? structuredClone(demoConnectedAccounts().find(a => a.account.provider === provider)?.report ?? null) : null;
  if (kind == null) demoReadFailures.delete(provider);
  else demoReadFailures.set(provider, { kind, report: ['needs_relogin', 'not_signed_in', 'identity_changed', 'permission_denied'].includes(kind) ? null : report });
  void command<Snapshot>('get_state').then(s => emitDemo('state-changed', s));
});
function demoConnectedAccounts(): AccountState[] {
  const original=demoSettings.connections;
  const normalized:Connection[]=original.map(c=>({...c,mode:c.mode==='devin_app'?'devin_cli':['antigravity_cli','antigravity_stored','antigravity_cli_usage'].includes(c.mode??'')?'antigravity_app':c.mode}));
  return demoConnectedAccountsFor(normalized).map((s,i)=>{
    if(s.report&&original[i].mode==='devin_app'){s.report.source='devin_app';s.report.plan='Pro · sample desktop scope';}
    if(s.report&&original[i].mode==='antigravity_cli'){s.report.source='antigravity_cli';s.report.plan='Sample daily quota · existing CLI';}
    if(s.report&&original[i].mode==='antigravity_stored'){s.report.source='antigravity_stored';s.report.plan='Sample daily quota · IDE stored session';}
    if(s.report&&original[i].mode==='antigravity_cli_usage'){s.report.source='antigravity_cli_usage';s.report.plan=null;s.report.verifiedAt=null;s.report.quotas=[{id:'gemini-weekly',label:'Gemini · weekly',percent:0,value:null,unit:null,resetsAt:reset(168)},{id:'3p-weekly',label:'Third-party models · weekly',percent:0,value:null,unit:null,resetsAt:reset(168)}];s.account.email=null;}
    const failure = demoReadFailures.get(s.account.provider);
    if (failure) {
      s.report = failure.report;
      s.status = s.report ? 'stale' : 'faulted';
      s.fault = { kind: failure.kind, message: failure.kind === 'needs_relogin' ? 'The provider rejected the saved session. Recover through the official client.' : 'Sample temporary usage read failure. Automatic retry is active.', retryAfterSecs: failure.kind === 'rate_limited' ? 120 : null };
    }
    return s;
  });
}
function demoConnectedAccountsFor(connections:Connection[]): AccountState[] {
  return connections.map(c => ({ account: { id: c.id, provider: c.provider, label: c.login, email: c.provider === 'cursor' ? c.login : null, sources: [] }, status: 'ready', fault: null, report: { source: c.mode === 'muse_device' ? 'muse_device' : c.mode === 'devin_cli' ? 'devin_cli' : c.mode === 'antigravity_app' ? 'antigravity_local' : c.mode === 'muse_cli' ? 'muse_cli' : c.mode === 'grok_cli_renewal' ? 'grok_cli_renewal' : c.mode === 'grok_cli' ? 'grok_cli' : c.mode === 'cursor_app' ? 'cursor_dashboard' : c.mode === 'github_cli' ? 'copilot_internal' : 'provider', plan: c.mode === 'devin_cli' ? 'Pro · sample CLI scope' : c.mode === 'antigravity_app' ? 'Pro · sample app' : c.mode === 'muse_device' ? 'High · authorized key endpoint · sample' : c.mode === 'muse_cli' ? 'High · last observation · sample' : ['grok_cli','grok_cli_renewal'].includes(c.mode??'') ? 'SuperGrok · sample' : c.mode === 'cursor_app' ? 'Pro · sample' : c.provider === 'cursor' ? 'Enterprise · sample member' : c.mode === 'github_cli' ? 'Copilot Pro · sample' : c.provider === 'copilot' ? 'Personal billing · sample' : 'API scope · sample', observedAt: new Date().toISOString(), quotas: c.mode === 'devin_cli' ? (new URLSearchParams(location.search).get('devinQuota')==='hidden'?[{id:'devin_unmapped',label:'Reported quota field · window unverified',percent:null,value:100,unit:'raw %',resetsAt:null}]:[{id:'devin_weekly',label:'Weekly quota',percent:30,value:null,unit:null,resetsAt:reset(72)},{id:'devin_extra_balance',label:'Extra usage balance · USD',percent:null,value:1.25,unit:'USD',resetsAt:null}]) : c.mode === 'antigravity_app' ? [{id:'gemini-5h',label:'Gemini · 5 hours',percent:25,value:null,unit:null,resetsAt:reset(3)},{id:'gemini-weekly',label:'Gemini · weekly',percent:50,value:null,unit:null,resetsAt:reset(72)}] : ['muse_cli','muse_device'].includes(c.mode??'') ? [{id:'muse_window',label:'300-minute subscription window',percent:25,value:null,unit:null,resetsAt:reset(3)},{id:'muse_weekly',label:'Weekly subscription',percent:40,value:null,unit:null,resetsAt:reset(72)}] : ['grok_cli','grok_cli_renewal'].includes(c.mode??'') ? [{id:'grok_credit_usage',label:'Reported credit usage',percent:25,value:null,unit:null,resetsAt:reset(120)}] : c.mode === 'cursor_app' ? [
    { id: 'cursor_monthly', label: 'Monthly plan', percent: 25, value: null, unit: null, resetsAt: reset(360) },
    { id: 'cursor_auto', label: 'Auto', percent: 10, value: null, unit: null, resetsAt: reset(360) },
    { id: 'cursor_api', label: 'API', percent: 42, value: null, unit: null, resetsAt: reset(360) },
  ] : c.provider === 'cursor' ? [
    { id: 'on_demand_budget', label: 'On-demand budget', percent: 25, value: null, unit: null, resetsAt: null },
    { id: 'cursor_on_demand', label: 'Billing cycle · On-demand usage (reported)', percent: null, value: 25, unit: 'USD', resetsAt: null },
  ] : c.mode === 'github_cli' ? [{id:'copilot_premium_interactions',label:'Premium requests',percent:28,value:null,unit:null,resetsAt:reset(72)}] : c.provider === 'deepseek' ? [{id:'deepseek_USD_total_balance',label:'Available balance · USD',percent:null,value:12.5,unit:'USD',resetsAt:null}] : ['zai','kimi','opencode'].includes(c.provider) ? [{id:`${c.provider}_quota`,label:'5-hour quota',percent:20,value:null,unit:null,resetsAt:reset(2)}] : [{ id: 'premium_request', label: 'This month · Premium requests used (reported)', percent: null, value: 42, unit: 'requests', resetsAt: null }] } }));
}
export async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (demo) {
    if(name==='get_sources')return {revision:Date.now(),sources:['codex','claude','antigravity','grok','muse','cursor','copilot','devin','zai','deepseek','kimi','opencode'].map(provider=>({provider,quota:['codex','claude'].includes(provider)?'cli':provider==='muse'?'cli':['cursor','antigravity'].includes(provider)?'desktop':['copilot','grok','devin'].includes(provider)?'existing_session':'api_key',localHistory:['codex','claude','grok','kimi','muse'].includes(provider),balanceOnly:provider==='deepseek',note:'Sample discovery only; no file or credential access.',sessionDetected:['codex','claude','cursor','copilot','grok','antigravity','devin','muse'].includes(provider),historyDetected:['codex','claude','grok','kimi','muse'].includes(provider),historyEnabled:demoSettings.historySources[provider as keyof Settings['historySources']]??(provider==='codex'?demoSettings.codexEnabled:provider==='claude'?demoSettings.claudeEnabled:false),monitoredAccounts:fixture.accounts.filter(a=>a.account.provider===provider).length+demoSettings.connections.filter(c=>c.provider===provider).length}))} as T;
    if(name==='set_history_source'){if(args?.consent!==true)throw new Error('Change local monitoring explicitly.');const provider=String(args.provider)as keyof Settings['historySources'];if(!['codex','claude','grok','kimi','muse'].includes(provider))throw new Error('No measured local reader exists.');const old=demoSettings;demoSettings={...demoSettings,historySources:{...demoSettings.historySources,[provider]:args.enabled===true}};try{saveDemo();}catch(error){demoSettings=old;throw error;}if(provider==='muse'&&args.enabled===true){demoMuseHasRecords=true;try{localStorage.setItem('opentoken.demo.muse-records','yes');}catch{/* Sample records only. */}}emitDemo('spending-changed',await command('get_tracking'));emitDemo('sources-changed',await command('get_sources'));return{provider,settings:structuredClone(demoSettings),connected:0,historyEnabled:args.enabled===true}as T;}
    if(name==='connect_grok_cli_renewal'){
      if(args?.consent!==true)throw new Error('Approve repeated Grok CLI renewal explicitly.');
      if(!window.confirm('Allow the official Grok CLI to renew the saved session after authentication rejection? Background grok models may change its credentials or request authentication. No model prompts or automatic extra login commands. Permission persists until disconnected or replaced using Use existing Grok.'))throw new Error('Grok CLI renewal cancelled. No command was run and no permission was saved.');
      const c=demoSettings.connections.find(c=>c.provider==='grok');if(!c)throw new Error('Connect Grok first, then approve CLI renewal for that saved account.');
      const scenario=new URLSearchParams(location.search).get('grokRenewal');
      if(scenario==='waiting')await new Promise<void>((resolve,reject)=>demoExistingWaiters.set('grok',{resolve,reject}));
      if(scenario==='error')throw new Error('The official Grok models check could not restore the session. No additional login command or model prompt was started.');
      const old=demoSettings;demoSettings={...demoSettings,connections:demoSettings.connections.map(old=>old.id===c.id?{...old,mode:'grok_cli_renewal' as const}:old)};try{saveDemo();}catch(error){demoSettings=old;throw error;}
      return{provider:'grok',settings:structuredClone(demoSettings),connected:1,historyEnabled:false}as T;
    }
    if(name==='connect_antigravity_cli_usage'){
      if(args?.consent!==true)throw new Error('Approve CLI-profile monitoring explicitly. No CLI was started.');
      if(!window.confirm('Monitor the current agy CLI profile, not a verified human account? Background quota-only commands may renew the official CLI session until disconnected. No model prompt or credential copy.'))throw new Error('CLI-profile monitoring cancelled. No CLI was started.');
      const scenario=new URLSearchParams(location.search).get('antigravityCli');
      if(scenario==='waiting')await new Promise<void>((resolve,reject)=>demoExistingWaiters.set('antigravity',{resolve,reject}));
      if(scenario==='error')throw new Error('The official CLI did not report recognized quota. No account or missing windows were inferred.');
      const old=demoSettings;const c:Connection={id:'demo-antigravity-cli-usage',provider:'antigravity',subject:'cli-context:sample',login:'Antigravity CLI · current profile',scope:'local:antigravity:cli-sample',mode:'antigravity_cli_usage',profile:'C:\\Preview\\agy\\agy.exe'};
      demoSettings={...demoSettings,connections:[...demoSettings.connections.filter(c=>c.mode!=='antigravity_cli_usage'),c]};try{saveDemo();}catch(error){demoSettings=old;throw error;}
      return{provider:'antigravity',settings:structuredClone(demoSettings),connected:1,historyEnabled:false}as T;
    }
    if(name==='connect_muse_subscription'){
      if(args?.consent!==true)throw new Error('Approve the key endpoint explicitly. Nothing was requested.');
      if(!window.confirm('Not read-only: Meta may issue or rotate a Muse CLI inference key. Replacement effects are undocumented and could affect running sessions. Background monitoring repeats this endpoint until disconnected. Returned keys are discarded and Muse credentials are never rewritten. Allow?'))throw new Error('Muse key endpoint access cancelled. No request was made.');
      if(new URLSearchParams(location.search).get('museKey')==='empty')throw new Error('Meta did not include subscription quota. No full allowance was inferred.');
      const old=demoSettings;const c:Connection={id:'demo-muse-device',provider:'muse',subject:'sample-muse-device',login:'muse@example.test',scope:'local:muse:sample',mode:'muse_device',profile:'C:\\Preview\\Muse\\fixture-profile'};
      demoSettings={...demoSettings,connections:[...demoSettings.connections.filter(c=>c.provider!=='muse'),c]};try{saveDemo();}catch(error){demoSettings=old;throw error;}
      return {settings:structuredClone(demoSettings),provider:'muse',connected:1,historyEnabled:demoSettings.historySources.muse??false} as T;
    }
    if(name==='cancel_existing_usage'){const provider=String(args?.provider);const waiter=demoExistingWaiters.get(provider);if(waiter){demoExistingWaiters.delete(provider);waiter.reject(new Error('Source checking cancelled. No new account was connected.'));}return undefined as T;}
    if(name==='connect_existing_usage'){
      if(args?.consent!==true)throw new Error('Allow read-only existing-source access explicitly.');
      const provider=String(args.provider);if(new URLSearchParams(location.search).get(`${provider}Existing`)==='missing')throw new Error('No usable existing session was found. No login was started.');
      if(['antigravity','devin','muse'].includes(provider)){
        const scenario=new URLSearchParams(location.search).get(`${provider}Existing`);
        if(scenario==='waiting')await new Promise<void>((resolve,reject)=>demoExistingWaiters.set(provider,{resolve,reject}));
        if(scenario==='empty')throw new Error('Muse has no subscription observation for this host yet. No prompt, login or key-mint fallback was requested.');
        if(scenario==='denied')throw new Error('This provider did not permit the usage read. No credential was sent to an alternative host.');
        const old=demoSettings;const p=provider as 'antigravity'|'devin'|'muse';const c:Connection={id:`demo-${p}-local`,provider:p,subject:`sample-${p}-scope`,login:p==='antigravity'?'antigravity@example.test':`${p==='devin'?'Devin':'Muse'} CLI · sample scope`,scope:`local:${p}:sample`,mode:p==='antigravity'?(new URLSearchParams(location.search).get('antigravitySource')==='cli'?'antigravity_cli':new URLSearchParams(location.search).get('antigravitySource')==='stored'?'antigravity_stored':'antigravity_app'):p==='devin'?(new URLSearchParams(location.search).get('devinSource')==='app'?'devin_app':'devin_cli'):'muse_cli',profile:`C:\\Preview\\${p}\\fixture-profile`};
        demoSettings={...demoSettings,connections:[...demoSettings.connections.filter(old=>old.provider!==p),c]};try{saveDemo();}catch(error){demoSettings=old;throw error;}
      }
      else if(provider==='cursor')await command('connect_cursor_personal',{consent:true});
      else if(provider==='copilot'){const old=demoSettings;const c:Connection={id:'demo-copilot-cli',provider:'copilot',subject:'101',login:'github-sample',scope:'gh:sample',mode:'github_cli',profile:'C:\\Preview\\GitHub CLI'};demoSettings={...demoSettings,connections:[...demoSettings.connections.filter(c=>c.mode!=='github_cli'),c]};try{saveDemo();}catch(error){demoSettings=old;throw error;}}
      else if(provider==='grok'){const old=demoSettings;const c:Connection={id:'demo-grok-cli',provider:'grok',subject:'sample-grok',login:'grok@example.test',scope:'grok:sample',mode:'grok_cli',profile:'C:\\Preview\\Grok'};demoSettings={...demoSettings,connections:[...demoSettings.connections.filter(c=>!['grok_cli','grok_cli_renewal'].includes(c.mode??'')),c]};try{saveDemo();}catch(error){demoSettings=old;throw error;}}
      else if(provider==='codex'||provider==='claude'){const old=demoSettings;demoSettings={...demoSettings,...(provider==='codex'?{codexEnabled:true}:{claudeEnabled:true})};try{saveDemo();}catch(error){demoSettings=old;throw error;}}
      else throw new Error('An existing session reader is not implemented for this capability.');
      return {provider,settings:structuredClone(demoSettings),connected:provider==='codex'?2:1,historyEnabled:['codex','claude'].includes(provider)} as T;
    }
    if (name === 'get_state') return { ...structuredClone(fixture), revision: Date.now(), accounts: [...structuredClone(fixture.accounts), ...structuredClone(demoCliAccounts), ...demoConnectedAccounts()].filter(a => !demoExcluded.includes(a.account.id)).reverse() } as T;
    if (name === 'get_settings') return structuredClone(demoSettings) as T;
    if (name === 'save_settings') { const previous = demoSettings; demoSettings = { ...args?.settings as Settings, connections: demoSettings.connections }; try { saveDemo(); } catch (e) { demoSettings = previous; throw e; } return structuredClone(demoSettings) as T; }
    if (name === 'get_cli_login_status' || name === 'start_cli_sign_in' || name === 'cancel_cli_sign_in') {
      const provider = args?.provider as CliProvider;
      if (provider !== 'codex' && provider !== 'claude') throw new Error('Choose a supported CLI provider.');
      if (name === 'start_cli_sign_in') {
        if (args?.consent !== true) throw new Error('Start sign-in explicitly.');
        if (!['checking', 'waiting', 'connecting'].includes(demoCliLogin[provider].phase)) {
          const run = ++demoCliRuns[provider]; const label = provider === 'codex' ? 'Codex' : 'Claude';
          demoCliPhase(provider, 'checking', `Checking ${label} sign-in…`);
          const scenario = new URLSearchParams(location.search).get(`${provider}Login`);
          if (scenario === 'waiting') demoCliPhase(provider, 'waiting', 'Finish in the official sign-in window. We’ll connect automatically.');
          else if (scenario === 'error') demoCliPhase(provider, 'error', `${label} CLI was not found. Install it from the official site, then try again.`, 'tool_missing');
          else setTimeout(() => void finishDemoCli(provider, run), 50);
        }
      } else if (name === 'cancel_cli_sign_in' && ['checking', 'waiting'].includes(demoCliLogin[provider].phase)) { ++demoCliRuns[provider]; demoCliPhase(provider, 'cancelled', 'Sign-in cancelled.'); }
      return structuredClone(demoCliLogin[provider]) as T;
    }
    if (name === 'disconnect_cli_account') {
      const id = String(args?.id);
      const account = [...fixture.accounts, ...demoCliAccounts].find(a => a.account.id === id);
      if (!account || !['codex', 'claude'].includes(account.account.provider)) throw new Error('That CLI account is no longer monitored.');
      const provider = account.account.provider as CliProvider;
      demoExcluded = [...new Set([...demoExcluded, id])]; localStorage.setItem('opentoken.demo.excluded', JSON.stringify(demoExcluded));
      demoSettings = { ...demoSettings, ignoredHomes: [...demoSettings.ignoredHomes, ...account.account.sources.map(s => s.path)] }; saveDemo();
      ++demoCliRuns[provider]; demoCliPhase(provider, 'idle', ''); emitDemo('cli-accounts-changed', { provider, settings: demoSettings });
      return structuredClone(demoSettings) as T;
    }
    if(name==='get_antigravity_login_status')return structuredClone(demoAntigravityLogin) as T;
    if(name==='start_antigravity_sign_in'){if(args?.consent!==true||!['ide','desktop','cli'].includes(String(args.client)))throw new Error('Choose official Antigravity recovery explicitly.');if(!['checking','waiting','connecting'].includes(demoAntigravityLogin.phase)){const run=++demoAntigravityRun;const scenario=new URLSearchParams(location.search).get('antigravityRecovery');if(scenario==='error')demoAntigravityPhase('error','The selected official client was not found. Nothing was installed.','tool_missing');else if(scenario==='waiting')demoAntigravityPhase('waiting','Finish recovery in the selected official client; no prompt is sent.');else setTimeout(()=>finishDemoAntigravity(run),50);if(scenario!=='error'&&scenario!=='waiting')demoAntigravityPhase('checking','Checking selected Antigravity client…');}return structuredClone(demoAntigravityLogin) as T;}
    if(name==='cancel_antigravity_sign_in'){if(['checking','waiting'].includes(demoAntigravityLogin.phase)){demoAntigravityRun++;demoAntigravityPhase('cancelled','Recovery cancelled.');}return structuredClone(demoAntigravityLogin) as T;}
    if(name==='get_grok_login_status')return structuredClone(demoGrokLogin) as T;
    if(name==='start_grok_sign_in'){
      if(args?.consent!==true)throw new Error('Start Grok sign-in explicitly.');
      if(args.recover===true&&args.deviceAuth===true)throw new Error('Choose either session recovery or device-code sign-in.');
      if(!['checking','waiting','connecting'].includes(demoGrokLogin.phase)){
        const run=++demoGrokRun;demoGrokPhase('checking','Checking Grok sign-in…');
        const scenario=new URLSearchParams(location.search).get(args.recover===true?'grokRecovery':'grokLogin');
        if(scenario==='waiting')demoGrokPhase('waiting',args.recover===true?'Finish the official Grok models check. No inference or automatic new OAuth fallback.':args.deviceAuth===true?'In the Grok terminal, find the device code. Enter that code on the official page Grok opens, then approve sign-in. Leave the terminal open; no browser return code needs to be pasted into OpenToken.':'Leave the Grok terminal open. Paste the FULL return URL at “Paste the URL here if it doesn’t connect” and press Enter—not just the code.');
        else if(scenario==='error')demoGrokPhase('error','Grok CLI not found.','tool_missing');
        else if(scenario==='rejected')demoGrokPhase('error','The official Grok model check did not restore an eligible OAuth session. Choose Sign in with Grok explicitly. No new login was launched.','permission_denied');
        else setTimeout(()=>finishDemoGrok(run),50);
      }return structuredClone(demoGrokLogin) as T;
    }
    if(name==='cancel_grok_sign_in'){if(['checking','waiting'].includes(demoGrokLogin.phase)){++demoGrokRun;demoGrokPhase('cancelled','Sign-in cancelled.');}return structuredClone(demoGrokLogin) as T;}
    if (name === 'get_github_login_status') return structuredClone(demoGithubLogin) as T;
    if (name === 'start_github_sign_in') {
      if(args?.consent !== true) throw new Error('Start GitHub sign-in explicitly.');
      if(!['checking','waiting','connecting'].includes(demoGithubLogin.phase)) {const run=++demoGithubRun;demoGithubPhase('checking','Checking Copilot sign-in…');const scenario=new URLSearchParams(location.search).get('githubLogin');if(scenario==='waiting')demoGithubPhase('waiting','Finish GitHub sign-in; we connect automatically.');else if(scenario==='error')demoGithubPhase('error','GitHub CLI not found.','tool_missing');else setTimeout(()=>finishDemoGithub(run),50);}
      return structuredClone(demoGithubLogin) as T;
    }
    if (name === 'cancel_github_sign_in') {if(['checking','waiting'].includes(demoGithubLogin.phase)){++demoGithubRun;demoGithubPhase('cancelled','Sign-in cancelled.');}return structuredClone(demoGithubLogin) as T;}
    if (name === 'get_cursor_login_status') return structuredClone(demoCursorLogin) as T;
    if (name === 'start_cursor_sign_in') {
      if (args?.consent !== true) throw new Error('Start sign-in explicitly to request read-only access.');
      if (['checking', 'waiting', 'connecting'].includes(demoCursorLogin.phase)) return structuredClone(demoCursorLogin) as T;
      const run = ++demoCursorRun; demoLoginPhase('checking', 'Checking Cursor sign-in…');
      const scenario = new URLSearchParams(location.search).get('cursorLogin');
      if (scenario === 'waiting') demoLoginPhase('waiting', 'Finish signing in in Cursor. We’ll connect automatically.');
      else if (scenario === 'error') setTimeout(() => { if (run === demoCursorRun) demoLoginPhase('error', 'Cursor wasn’t found. Install it or open your custom installation manually.', 'tool_missing'); }, 50);
      else setTimeout(() => void finishDemoCursorLogin(run), 50);
      return structuredClone(demoCursorLogin) as T;
    }
    if (name === 'cancel_cursor_sign_in') {
      if (['checking', 'waiting'].includes(demoCursorLogin.phase)) { ++demoCursorRun; demoLoginPhase('cancelled', 'Sign-in cancelled.'); }
      return structuredClone(demoCursorLogin) as T;
    }
    if (name === 'connect_cursor_personal') {
      if (args?.consent !== true) throw new Error('Allow read-only access before connecting.');
      const connection: Connection = { id: 'demo-cursor-app', provider: 'cursor', subject: 'sample-personal', login: 'cursor-personal@example.test', scope: 'app:sample-profile', mode: 'cursor_app', profile: 'C:\\Preview\\Cursor\\User\\globalStorage\\state.vscdb' };
      const previous = demoSettings; demoSettings = { ...demoSettings, connections: [...demoSettings.connections.filter(c => c.mode !== 'cursor_app'), connection] };
      try { saveDemo(); } catch (e) { demoSettings = previous; throw e; } return structuredClone(demoSettings) as T;
    }
    if (name === 'connect_provider') {
      const input = args?.input as { provider: string; token: string; memberEmail: string; region?: string };
      if (input.token !== 'opentoken-demo-token' || !['cursor', 'copilot', 'zai', 'deepseek', 'kimi', 'opencode'].includes(input.provider)) throw new Error('Sample verification failed. Preview accepts only opentoken-demo-token; no real credential was sent or saved.');
      if (input.provider === 'cursor' && !/^[^@\s]+@[^@\s]+$/.test(input.memberEmail)) throw new Error('Enter the exact Cursor team member email.');
      const provider = input.provider as Connection['provider'];
      const connection: Connection = { id: `demo-${provider}`, provider, subject: 'sample-member', login: provider === 'cursor' ? input.memberEmail : provider === 'copilot' ? 'sample-user' : `${provider} · sample API key`, scope: provider === 'cursor' ? 'sample-team' : provider === 'copilot' ? 'personal' : `${input.region || 'global'}:sample-key` };
      const previous = demoSettings; demoSettings = { ...demoSettings, connections: [...demoSettings.connections.filter(c => c.id !== connection.id),connection] };
      try { saveDemo(); } catch (e) { demoSettings = previous; throw e; } return structuredClone(demoSettings) as T;
    }
    if (name === 'disconnect_provider') {
      const previous = demoSettings; demoSettings = { ...demoSettings, connections: demoSettings.connections.filter(c => c.id !== args?.id) };
      try { saveDemo(); } catch (e) { demoSettings = previous; throw e; }
      if(previous.connections.some(c=>c.id===args?.id&&['grok_cli','grok_cli_renewal'].includes(c.mode??''))){++demoGrokRun;demoGrokPhase('idle','');}
      if(previous.connections.some(c=>c.id===args?.id&&c.mode==='github_cli')){++demoGithubRun;demoGithubPhase('idle','');}
      if (previous.connections.some(c => c.id === args?.id && c.mode === 'cursor_app')) { ++demoCursorRun; demoLoginPhase('idle', ''); }
      return structuredClone(demoSettings) as T;
    }
    if (name === 'save_account_order') { demoSettings = { ...demoSettings, accountOrder: args?.order as string[] }; saveDemo(); return undefined as T; }
    if (name === 'set_dashboard_pinned') { const previous = demoSettings; demoSettings = { ...demoSettings, dashboardPinned: args?.pinned as boolean }; try { saveDemo(); } catch (e) { demoSettings = previous; throw e; } return undefined as T; }
    if (name === 'get_tracking') return { revision: Date.now(), mode: demoSettings.liveTracking ? 'watching' : 'off', data: await command('get_spending'), error: null, lastActivityAt: {} } as T;
    if (name === 'get_spending') return { updatedAt: new Date().toISOString(), indexing: false, filesIndexed: 0, filesPending: 0, skippedRecords: 0, pricingDate: '2026-10-14', referencePrices: {}, creditTrackingSince: null, unpricedModels: [], periods: Object.fromEntries(['Today', 'Yesterday', '30 Days'].map(p => [p, demoMuseHasRecords?{tokens:125,estimatedUsd:null,unpricedTokens:125,providers:[{provider:'muse',tokens:125,estimatedUsd:null,unpricedTokens:125}],creditChanges:[]}:{ tokens: 0, estimatedUsd: null, unpricedTokens: 0, providers: [], creditChanges: [] }])) } as T;
    if (name === 'refresh') return undefined as T;
    throw new Error('This action is only available in the desktop app.');
  }
  if (!isTauri()) throw new Error('OpenToken is a desktop app. Run npm run desktop:dev, or add ?demo to preview the interface.');
  return invoke<T>(name, args);
}
export async function subscribe<T>(event: string, handler: (payload: T) => void) {
  if (demo) {
    const listeners = demoListeners.get(event) ?? new Set<(payload: unknown) => void>();
    const receive = (payload: unknown) => handler(payload as T); listeners.add(receive); demoListeners.set(event, listeners);
    return () => { listeners.delete(receive); if (!listeners.size) demoListeners.delete(event); };
  }
  if (!isTauri()) return () => {};
  return listen<T>(event, e => handler(e.payload));
}
export async function hide() { if (isTauri()) await command('hide_dashboard'); }
export async function drag() { if (isTauri()) await getCurrentWindow().startDragging(); }
