import { useId } from 'react';
import { ExternalLink, LoaderCircle } from 'lucide-react';
import type { Action } from '../ui';
import { command, demo } from '../bridge';
import { cursorLoginActive, cursorLoginCancellable } from '../cursorLogin';
import useCliLogin, { type CliProvider } from '../useCliLogin';
import ProviderGlyph from './ProviderGlyph';
export default function CliConnection({ provider, busy, action }: { provider: CliProvider; busy: boolean; action: Action }) {
  const login = useCliLogin(provider);
  const hint = useId();
  const name = provider === 'codex' ? 'Codex' : 'Claude';
  const active = cursorLoginActive(login.status);
  const description = `Opens the official ${name} CLI sign-in. Account storage is handled automatically; existing sign-ins and credentials stay untouched.`;
  return <div className="cursor-sign-in cli-sign-in">
    <button className="primary cursor-sign-in-button" type="button" disabled={busy || active} title={description} aria-describedby={hint} onClick={() => void login.start()}>
      {active ? <LoaderCircle className="sign-in-spinner" size={17} aria-hidden="true"/> : <ProviderGlyph provider={provider} size={19}/>}
      {active ? 'Signing in…' : `Sign in with ${name}`}
    </button>
    <span className="visually-hidden" id={hint}>{description}</span>
    {demo && <small className="sign-in-demo">Sample connection · no real credentials</small>}
    {login.status.phase !== 'idle' && <p className={`sign-in-status ${login.status.phase === 'error' ? 'error' : ''}`} role="status" aria-label={`${name} sign-in status`}>{login.status.message}</p>}
    {cursorLoginCancellable(login.status) && <button className="text-button" type="button" onClick={() => void login.cancel()}>Cancel sign-in</button>}
    {login.status.phase === 'error' && login.status.failure === 'tool_missing' && <button className="text-button" type="button" disabled={busy} onClick={() => void action(() => command('open_provider_page', { provider, docs: false }))}><ExternalLink size={13}/>Get {name} CLI</button>}
  </div>;
}
