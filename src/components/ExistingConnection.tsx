import {useState,useId,useRef,type ReactNode} from 'react';
import {Search,LoaderCircle} from 'lucide-react';
import type {Settings,Provider} from '../types';
import type {Action} from '../ui';
import {command,demo} from '../bridge';
import {providerDefinition} from '../providers';

export default function ExistingConnection({provider,busy,action,changed,recovery,historyOnly=false,needsSignIn=0}:{provider:Provider;busy:boolean;action:Action;changed:(next:Settings)=>Promise<void>;recovery?:ReactNode;historyOnly?:boolean;needsSignIn?:number}){
 const [found,setFound]=useState('');
 const [recover,setRecover]=useState(!historyOnly&&needsSignIn>0);
 const [checking,setChecking]=useState(false);
 const cancelled=useRef(false);
 const name=providerDefinition(provider).name;
 const hint=useId();
 const description=(historyOnly?`Read recorded ${name} CLI tokens on this device. No credentials or account quota are inferred.`:
  provider==='cursor'?"Reuse your Cursor desktop sign-in read-only. Usage comes from an unofficial dashboard; Cursor owns its credentials.":
  provider==='copilot'?"Reuse your existing official GitHub CLI session for the unofficial Copilot quota API. Provider access is verified, not assumed from login.":
  provider==='grok'?"Reuse your official Grok CLI session for its private credit API. Grok owns token renewal.":
  provider==='devin'?"Reuse your existing Devin CLI credential or, when no CLI profile exists, the Devin desktop sign-in read-only. Only its exact authentication record is read; no database copy, browser data, foreign server or invented weekly allowance.":
  provider==='antigravity'?"Read your running Antigravity app or IDE after native ownership checks. If closed, check the IDE's exact persisted OAuth record before the separate agy CLI entry. The provider verifies current access; cache timestamps alone are not authentication. No vault enumeration, token copy, OAuth renewal or browser launch.":
  provider==='muse'?"Ask the installed official Muse CLI for a last-observed subscription reading. A fresh host may have none. The profile login is used only to bind scope; no prompt, session creation or key-mint fallback is requested.":
  `Use existing ${name} CLI account usage without creating another sign-in profile.`)+' Read existing authorized usage sources without starting login, installing software or copying credentials.';
 async function connect(){
  setFound('');cancelled.current=false;if(!historyOnly)setChecking(true);
  try{
   const result=await command<{settings:Settings;connected:number;historyEnabled:boolean}>(historyOnly?'set_history_source':'connect_existing_usage',{provider,consent:true,enabled:true});
   await changed(result.settings);
   setFound(result.connected?`${result.connected} existing ${name} account${result.connected===1?'':'s'} verified. Monitoring is active.`:'Local usage tracking enabled. No account login or quota was inferred.');
  }catch(error){
   if(cancelled.current&&String(error).includes('cancelled'))setFound('Source check cancelled. No new account was added.');
   else {if(recovery&&/rejected|needs recovery|expir/i.test(String(error)))setRecover(true);throw error;}
  }finally{setChecking(false);}
 }
 async function cancel(){cancelled.current=true;await command('cancel_existing_usage',{provider});}
 return <div className="cursor-sign-in existing-connection">
  <button className="primary cursor-sign-in-button" disabled={busy||checking} title={description} aria-describedby={hint} onClick={()=>void action(connect)}>{checking?<LoaderCircle className="sign-in-spinner" size={17}/>:<Search size={17}/>} {checking?'Checking existing usage…':historyOnly?'Track local usage':`Use existing ${name}`}</button>
  <span className="visually-hidden" id={hint}>{description}</span>
  {checking&&<button className="text-button" onClick={()=>void cancel()}>Cancel source check</button>}
  {demo&&<small className="sign-in-demo">Sample source · no real credentials</small>}
  {found&&<p role="status" aria-label={`${name} source status`}>{found}</p>}
  {!historyOnly&&needsSignIn>0&&<p className="sign-in-needed" role="status">{needsSignIn===1?`1 ${name} account needs you to sign in again.`:`${needsSignIn} ${name} accounts need you to sign in again.`}{recovery?' Use the sign-in below.':provider==='antigravity'?' Use “Sign in or recover session” below, then check again.':` Log in with the official ${name} app or CLI, then press “Use existing ${name}”.`}</p>}
  {!historyOnly&&!recovery&&!needsSignIn&&(provider==='devin'||provider==='muse')&&<p className="muted-text">Not signed in yet? Log in with the official {name} app or CLI first, then press “Use existing {name}”.</p>}
  {recovery&&<><button className="connection-advanced-toggle" disabled={busy||checking} onClick={()=>setRecover(!recover)}>{recover?'Hide sign-in options':'Sign in or add an account'}</button>{recover&&recovery}</>}
 </div>;
}
