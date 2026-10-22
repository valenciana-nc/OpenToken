import {useState} from 'react';
import {command,demo} from '../bridge';
import type {Settings} from '../types';
import type {Action} from '../ui';
import useGithubLogin from '../useGithubLogin';
import {cursorLoginActive,cursorLoginCancellable} from '../cursorLogin';
export default function AntigravityRecovery({busy,action,changed}:{busy:boolean;action:Action;changed:(s:Settings)=>Promise<void>}){
 const recovery=useGithubLogin(true,'antigravity');const active=cursorLoginActive(recovery.status);const[checking,setChecking]=useState(false);const[message,setMessage]=useState('');
 async function monitor(){setChecking(true);setMessage('Checking the official CLI quota report…');try{const result=await command<{settings:Settings}>('connect_antigravity_cli_usage',{consent:true});await changed(result.settings);setMessage('CLI-profile monitoring enabled. Account identity is not provided.');}catch(error){setMessage(String(error));throw error;}finally{setChecking(false);}}
 return <><div className="cursor-sign-in"><button className="primary cursor-sign-in-button" disabled={busy||active||checking} title="Monitor quota reported by your installed agy CLI's current profile, not a verified person. The native confirmation explains repeated background calls and possible owner-side login renewal. No model prompt or credential copy." onClick={()=>void action(monitor)}>Monitor with agy CLI</button>{demo&&<small className="sign-in-demo">Sample CLI-profile quota · no real CLI is run</small>}{message&&<p role="status" aria-label="Antigravity CLI profile status">{message}</p>}{checking&&<button className="text-button" onClick={()=>void command('cancel_existing_usage',{provider:'antigravity'})}>Cancel CLI-profile check</button>}</div><details className="connection-context"><summary>Session recovery (only if needed)</summary>
  {([['ide','Antigravity IDE'],['desktop','Antigravity app'],['cli','agy CLI']] as const).map(([client,name])=><button key={client} className="text-button" disabled={busy||active||checking} title={client==='cli'?'Explicit recovery: run only the installed official agy models command. The CLI may renew its own access or ask you to authenticate. No model prompt is sent; OpenToken verifies quota afterward.':'Explicit recovery: open this installed official client. It owns authentication and may ask you to sign in. OpenToken verifies that selected client’s account and quota; it does not renew Google grants or close the client on cancellation.'} onClick={()=>void action(()=>recovery.start({client}))}>Recover with {name}</button>)}
  {recovery.status.phase!=='idle'&&<p role="status" aria-label="Antigravity recovery status">{recovery.status.message}</p>}
  {cursorLoginCancellable(recovery.status)&&<button className="text-button" onClick={()=>void recovery.cancel()}>Cancel recovery</button>}
 </details></>;
}
