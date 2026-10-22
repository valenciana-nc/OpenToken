import {useState} from 'react';
import {LoaderCircle} from 'lucide-react';
import {command,demo} from '../bridge';
import type {Settings} from '../types';
import type {Action} from '../ui';
export default function MuseSubscription({busy,action,changed}:{busy:boolean;action:Action;changed:(settings:Settings)=>Promise<void>}){
 const[open,setOpen]=useState(false);const[checking,setChecking]=useState(false);const[message,setMessage]=useState('');
 async function connect(){setChecking(true);setMessage('');try{const result=await command<{settings:Settings}>('connect_muse_subscription',{consent:true});await changed(result.settings);setMessage('Muse subscription monitoring enabled. The authorized key endpoint is used until disconnected.');}finally{setChecking(false);}}
 return <div className="cursor-sign-in"><button className="connection-advanced-toggle" disabled={busy||checking} onClick={()=>setOpen(!open)}>{open?'Hide advanced subscription refresh':'Advanced subscription refresh'}</button>{open&&<div className="provider-connection-options">
  <p className="connection-scope">Not read-only · Meta may issue or rotate a Muse CLI inference key. Replacement effects are undocumented and could affect running sessions. Background refresh repeats this endpoint until disconnected.</p>
  <p className="hint">OpenToken discards returned inference keys and never writes Muse’s credential file, sends a prompt or changes payment settings. The native confirmation defaults to No. Cancelling or a failed save cannot undo a request already sent.</p>
  {demo&&<small className="sign-in-demo">Sample endpoint · no real key is issued</small>}
  <button className="primary cursor-sign-in-button" disabled={busy||checking} onClick={()=>void action(connect)}>{checking&&<LoaderCircle className="sign-in-spinner" size={17}/>}Review key endpoint &amp; connect</button>
  {checking&&<button className="text-button" onClick={()=>void command('cancel_existing_usage',{provider:'muse'})}>Cancel source check</button>}
  {message&&<p role="status" aria-label="Muse subscription status">{message}</p>}
 </div>}</div>;
}
