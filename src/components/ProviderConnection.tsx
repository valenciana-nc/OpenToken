import { useId, useState } from 'react';
import { ChevronLeft, ChevronRight, ExternalLink, Info, Link, LoaderCircle } from 'lucide-react';
import type { AccountState, Settings } from '../types';
import { signInNeeded } from '../monitoringHealth';
import type { Action } from '../ui';
import { command, demo } from '../bridge';
import { connectionScope, providerDefinition, type ProviderId } from '../providers';
import { cursorLoginActive, cursorLoginCancellable } from '../cursorLogin';
import useCursorLogin from '../useCursorLogin';
import useGithubLogin from '../useGithubLogin';
import ProviderGlyph from './ProviderGlyph';
import CliConnection from './CliConnection';
import ExistingConnection from './ExistingConnection';
import MuseSubscription from './MuseSubscription';
import AntigravityRecovery from './AntigravityRecovery';
import GrokConnection from './GrokConnection';

type Props={provider:ProviderId;busy:boolean;action:Action;changed:(next:Settings)=>Promise<void>;accounts?:AccountState[]};
function AuthenticationOptions({ provider, busy, action, changed,initialMode,back,hideAdvanced=false }: Props&{initialMode?:'enterprise'|'billing';back?:()=>void;hideAdvanced?:boolean}) {
  const [token, setToken] = useState('');
  const [email, setEmail] = useState('');
  const [enterprise, setEnterprise] = useState(initialMode==='enterprise');
  const [copilotBilling, setCopilotBilling] = useState(initialMode==='billing');
  const [region, setRegion] = useState(provider === 'kimi' ? 'cn' : 'global');
  const hint = useId();
  const login = useCursorLogin(provider === 'cursor');
  const signingIn = cursorLoginActive(login.status);
  const github = useGithubLogin(provider === 'copilot');
  const githubActive = cursorLoginActive(github.status);
  const name = providerDefinition(provider).name;
  const apiKeyProvider = ['zai', 'deepseek', 'kimi', 'opencode'].includes(provider);
  const supported = provider === 'cursor' || provider === 'copilot' || provider === 'grok' || apiKeyProvider;
  const cursorDescription = 'Use your existing Cursor desktop sign-in read-only until disconnected. Usage comes from an unofficial dashboard; Cursor keeps ownership of its tokens.';
  if (provider === 'codex' || provider === 'claude') return <CliConnection provider={provider} busy={busy} action={action}/>;
  if (!supported) return <div className="provider-empty"><h3>{name} connection options</h3><p>OpenToken doesn’t have a live {name} connector yet. No credentials are requested and no usage or cost data is shown for unsupported providers.</p><p>{provider === 'opencode' ? 'OpenCode is a multi-provider client, not a subscription quota provider. Its local history needs a separate, deduplicated importer before we can report recorded tokens.' : 'A connector needs a supported authentication method and a verified usage source. Signing in alone does not prove remaining allowance.'}</p></div>;
  async function connect() {
    const submitted = token; setToken('');
    const next = await command<Settings>('connect_provider', { input: { provider, token: submitted, memberEmail: email.trim(), region } });
    await changed(next);
  }
  function toggleEnterprise() { setToken(''); setEmail(''); if(back)back();else setEnterprise(value => !value); }
  if (provider === 'grok') return <GrokConnection busy={busy} action={action} changed={changed}/>;
  if (provider === 'copilot' && !copilotBilling) return <div className="cursor-sign-in">
    <button className="primary cursor-sign-in-button" disabled={busy || githubActive} type="button" title="Reuse the official GitHub CLI login, or sign in through its window/browser. GitHub CLI owns credentials and requests its normal permissions. OpenToken only reads identity and the unofficial Copilot quota endpoint." onClick={() => void github.start()}>{githubActive ? <LoaderCircle className="sign-in-spinner" size={17}/> : <ProviderGlyph provider="copilot" size={19}/>} {githubActive ? 'Signing in…' : 'Sign in with GitHub'}</button>
    {demo && <small className="sign-in-demo">Sample connection · no real credentials</small>}
    {github.status.phase !== 'idle' && <p className={`sign-in-status ${github.status.phase === 'error' ? 'error' : ''}`} role="status" aria-label="Copilot sign-in status">{github.status.message}</p>}
    {cursorLoginCancellable(github.status) && <button className="text-button" onClick={() => void github.cancel()}>Cancel sign-in</button>}
    {github.status.failure === 'tool_missing' && <button className="text-button" onClick={() => void action(() => command('open_provider_page', { provider:'github_cli',docs:false }))}>Get GitHub CLI</button>}
    {!hideAdvanced&&<button className="connection-advanced-toggle" disabled={busy || githubActive} onClick={() => { setToken('');setCopilotBilling(true); }}>Advanced personal billing<ChevronRight size={12}/></button>}
  </div>;
  if (provider === 'cursor' && !enterprise) return <div className="cursor-sign-in">
    <button className="primary cursor-sign-in-button" type="button" disabled={busy || signingIn} title={cursorDescription} aria-describedby={hint} onClick={() => void login.start()}>
      {signingIn ? <LoaderCircle className="sign-in-spinner" size={17} aria-hidden="true"/> : <ProviderGlyph provider="cursor" size={19}/>}
      {signingIn ? 'Signing in…' : 'Sign in with Cursor'}
    </button>
    <span className="visually-hidden" id={hint}>{cursorDescription}</span>
    {demo && <small className="sign-in-demo">Sample connection · no real credentials</small>}
    {login.status.phase !== 'idle' && <p className={`sign-in-status ${login.status.phase === 'error' ? 'error' : ''}`} role="status" aria-label="Cursor sign-in status">{login.status.message}</p>}
    {cursorLoginCancellable(login.status) && <button className="text-button" type="button" onClick={() => void login.cancel()}>Cancel sign-in</button>}
    {login.status.phase === 'error' && login.status.failure === 'tool_missing' && <button className="text-button" type="button" disabled={busy} onClick={() => void action(() => command('open_provider_page', { provider: 'cursor_personal', docs: false }))}><ExternalLink size={13}/>Get Cursor</button>}
    {!hideAdvanced&&<button className="connection-advanced-toggle" type="button" disabled={busy || signingIn} onClick={toggleEnterprise}>Enterprise setup<ChevronRight size={12} aria-hidden="true"/></button>}
  </div>;
  return <div className="provider-connection-options">
    {provider === 'cursor' && <h3>Cursor Enterprise</h3>}
    <form className="provider-connection" onSubmit={e => { e.preventDefault(); void action(connect, `${name} connected. ${apiKeyProvider ? 'API-key scope and reported usage' : 'Identity and available usage'} were verified.`); }}>
      <p className="connection-scope">{provider === 'cursor' ? 'Advanced · selected Enterprise member' : provider === 'copilot' ? 'Personal billing · Plan: read token' : provider === 'deepseek' ? 'API balance · not DeepSeek Chat quota' : provider === 'opencode' ? 'OpenCode Go quota · not every client session' : 'Coding Plan quota · API-key scope'}</p>
      <details className="connection-context"><summary><Info size={12} aria-hidden="true"/>About this connection</summary><p className="connection-scope">{connectionScope(provider)}</p><p>{apiKeyProvider ? 'Create a key for this provider and region, then connect. The key authenticates its API scope, not an inferred person or email. No missing limit, token history or price is invented. Stored only in Windows Credential Manager after a successful usage read.' : provider === 'cursor' ? 'Create an Enterprise admin API key with admin:* scope in Cursor’s dashboard. This key can have broader administrative privileges; OpenToken only calls the documented read endpoints. Choose the exact member email—an admin key does not identify its owner.' : 'Create a fine-grained GitHub personal access token for your personal account with only the user permission Plan: read. No repository permissions are needed. Classic tokens, an ordinary GitHub CLI login, and organization seats do not provide this personal billing connection.'}</p></details>
      <button className="text-button" type="button" disabled={busy} onClick={() => void action(() => command('open_provider_page', { provider: provider === 'zai' && region === 'cn' ? 'zai_cn' : provider === 'kimi' && region === 'international' ? 'kimi_international' : provider, docs: false }))}><ExternalLink size={15}/>Open {provider === 'copilot' ? 'GitHub token settings' : `${name} API key settings`}</button>

      {(provider === 'zai' || provider === 'kimi') && <label>Region<select value={region} disabled={busy} onChange={e => { setRegion(e.target.value); setToken(''); }}><option value={provider === 'kimi' ? 'cn' : 'global'}>{provider === 'kimi' ? 'China · kimi.com' : 'Global · api.z.ai'}</option><option value={provider === 'kimi' ? 'international' : 'cn'}>{provider === 'kimi' ? 'International · kimi.ai' : 'China · BigModel'}</option></select></label>}
      {provider === 'cursor' && <label>Team member email<input type="email" value={email} maxLength={254} required disabled={busy} onChange={e => setEmail(e.target.value)} placeholder="you@company.com"/></label>}
      <label>{provider === 'cursor' ? 'Enterprise admin API key' : provider === 'copilot' ? 'Fine-grained personal access token' : `${name === 'Kimi' ? 'Kimi Code' : name === 'OpenCode' ? 'OpenCode Go' : name} API key`}<input type="password" autoComplete="off" spellCheck={false} autoCapitalize="none" value={token} minLength={16} maxLength={2048} required disabled={busy} aria-describedby={hint} onChange={e => setToken(e.target.value)}/></label>
      <details className="connection-context"><summary>Credential storage</summary><p className="hint" id={hint}>{demo ? 'Design preview: never enter real credentials here. Use opentoken-demo-token to exercise the sample flow.' : 'Verified before saving to Windows Credential Manager. The secret field is cleared on submission; credentials are never saved in preferences, usage reports or logs. Disconnect removes only OpenToken’s saved credential, not your provider login.'}</p></details>
      <button className="primary full" type="submit" disabled={busy}><Link size={16}/>{busy ? 'Verifying connection…' : 'Verify & connect'}</button>
    </form>
    {provider === 'cursor' && <button className="connection-advanced-toggle" type="button" disabled={busy} onClick={toggleEnterprise}><ChevronLeft size={12} aria-hidden="true"/>Back to personal sign-in</button>}
    {provider === 'copilot' && <button className="connection-advanced-toggle" disabled={busy} onClick={() => {setToken('');if(back)back();else setCopilotBilling(false);}}>Back to GitHub sign-in</button>}
  </div>;
}
export default function ProviderConnection(props:Props){const[advanced,setAdvanced]=useState<'enterprise'|'billing'|null>(null);const needs=signInNeeded(props.accounts??[],props.provider);if(advanced)return <AuthenticationOptions {...props} key={advanced} initialMode={advanced} back={()=>setAdvanced(null)}/>;if(['devin','antigravity','muse'].includes(props.provider))return <><ExistingConnection {...props} needsSignIn={needs}/>{props.provider==='antigravity'&&<AntigravityRecovery {...props} needsSignIn={needs}/>}{props.provider==='muse'&&<><ExistingConnection {...props} historyOnly/><MuseSubscription {...props}/></>}</>;if(props.provider==='kimi')return <><ExistingConnection {...props} historyOnly/><AuthenticationOptions {...props}/></>;if(['codex','claude','cursor','copilot','grok','muse'].includes(props.provider))return <><ExistingConnection {...props} needsSignIn={needs} recovery={<AuthenticationOptions {...props} hideAdvanced/>}/>{props.provider==='grok'&&<ExistingConnection {...props} historyOnly/>}{props.provider==='cursor'&&<button className="connection-advanced-toggle" disabled={props.busy} onClick={()=>setAdvanced('enterprise')}>Enterprise setup<ChevronRight size={12}/></button>}{props.provider==='copilot'&&<button className="connection-advanced-toggle" disabled={props.busy} onClick={()=>setAdvanced('billing')}>Advanced personal billing<ChevronRight size={12}/></button>}</>;return <AuthenticationOptions {...props}/>;}
