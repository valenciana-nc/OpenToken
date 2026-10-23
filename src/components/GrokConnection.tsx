import {useState} from 'react';
import {LoaderCircle} from 'lucide-react';
import type {Settings} from '../types';
import type {Action} from '../ui';
import {command,demo} from '../bridge';
import {cursorLoginActive,cursorLoginCancellable} from '../cursorLogin';
import useGithubLogin from '../useGithubLogin';
import ProviderGlyph from './ProviderGlyph';

export default function GrokConnection({busy,action,changed}:{busy:boolean;action:Action;changed:(settings:Settings)=>Promise<void>}) {
 const login=useGithubLogin(true,'grok');
 const active=cursorLoginActive(login.status);
 const [checking,setChecking]=useState(false);
 const [notice,setNotice]=useState('');
 async function renewal(){setNotice('');setChecking(true);try{const result=await command<{settings:Settings}>('connect_grok_cli_renewal',{consent:true});await changed(result.settings);setNotice('Official Grok CLI renewal enabled for this saved account. Disconnect or Use existing Grok to stop it.');}finally{setChecking(false);}}
 return <div className="cursor-sign-in">
  <button className="primary cursor-sign-in-button" disabled={busy||active||checking} title="Device-code sign-in using the installed official Grok CLI. Enter the terminal’s code on the official website; no callback code needs to be pasted into OpenToken." onClick={()=>void login.start({deviceAuth:true})}>{active?<LoaderCircle className="sign-in-spinner" size={17}/>:<ProviderGlyph provider="grok" size={19}/>} {active?'Signing in…':'Sign in with Grok'}</button>
  <p className="hint">Uses a device code: enter the code from the Grok terminal on the official page it opens. Keep that terminal open until sign-in finishes.</p>
  {demo&&<small className="sign-in-demo">Sample connection · no real credentials</small>}
  {login.status.phase!=='idle'&&<p role="status" aria-label="Grok sign-in status" className={`sign-in-status ${login.status.phase==='error'?'error':''}`}>{login.status.message}</p>}
  {cursorLoginCancellable(login.status)&&<button className="text-button" onClick={()=>void login.cancel()}>Cancel sign-in</button>}
  <details className="connection-context"><summary>Browser return-code help</summary><p>If you used browser sign-in, return to the <b>OpenToken - Grok sign-in</b> terminal (Alt+Tab). At <b>Paste the URL here if it doesn’t connect</b>, paste the <b>full return URL</b> from the browser and press Enter—not just the code. Never paste it into OpenToken or chat. If the terminal closed or the attempt expired, start a new sign-in.</p><button className="text-button" disabled={busy||active||checking} onClick={()=>void login.start({deviceAuth:false})}>Use browser sign-in instead</button></details>
  <button className="text-button" disabled={busy||active||checking} title="Run only grok models once. The official client may renew its own saved session. No model prompt or automatic additional login command." onClick={()=>void login.recover()}>Recover Grok session</button>
  <button className="text-button" disabled={busy||active||checking} onClick={()=>void action(renewal)}>Enable automatic Grok renewal…</button>
  <p className="hint">Grok access tokens expire. Read-only monitoring cannot renew them. Automatic renewal needs a separate native approval and lets the official CLI maintain the saved session—no model prompts.</p>
  {checking&&<button className="text-button" onClick={()=>void command('cancel_existing_usage',{provider:'grok'})}>Cancel renewal check</button>}
  {notice&&<p role="status" aria-label="Grok renewal status">{notice}</p>}
  {login.status.failure==='tool_missing'&&<button className="text-button" onClick={()=>void action(()=>command('open_provider_page',{provider:'grok',docs:false}))}>Get Grok CLI</button>}
 </div>;
}
