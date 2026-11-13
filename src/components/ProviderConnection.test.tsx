import{describe,expect,it,vi}from'vitest';import{renderToStaticMarkup}from'react-dom/server';import{defaultSettings,emptySnapshot,type AccountState}from'../types';import{isHistoryProvider,isTrackedProvider}from'../providers';
vi.mock('../bridge',()=>({command:vi.fn(),demo:false}));import ProviderConnection from'./ProviderConnection';import ProviderPanel from'./ProviderPanel';const props={busy:false,action:async()=>{},changed:async()=>{}};
describe('discovery-first provider capabilities',()=>{
 it.each(['codex','claude','cursor','copilot','grok']as const)('%s reuses existing usage before offering account setup',provider=>{const html=renderToStaticMarkup(<ProviderConnection {...props} provider={provider}/>);expect(html).toContain(`Use existing ${provider==='copilot'?'Copilot':provider[0].toUpperCase()+provider.slice(1)}`);expect(html).toContain('without starting login');expect(html).toContain('Sign in or add an account');expect(html).not.toContain('Sign in with');expect(html).not.toContain('<input');expect(html).not.toContain('Browse folders');expect(html).not.toContain('Login folder');expect(html).not.toContain('Tracked folders');expect(html).not.toContain('sign-in-caption');expect(html).not.toContain('sign-in-info');});
 it('Muse records tokens without a quota credential or login button',()=>{const html=renderToStaticMarkup(<ProviderConnection {...props} provider="muse"/>);expect(html).toContain('Track local usage');expect(html).not.toContain('<input');expect(html).not.toContain('Sign in');expect(html).not.toContain('repair sign-in');expect(isHistoryProvider('muse')).toBe(true);expect(html).toContain('Use existing Muse Code');expect(html).toContain('A fresh host may have none');expect(isTrackedProvider('muse')).toBe(true);});
 it.each(['zai','deepseek','kimi','opencode']as const)('%s keeps its actual key scope, not a fake OAuth button',provider=>{const html=renderToStaticMarkup(<ProviderConnection {...props} provider={provider}/>);expect(html).toContain('type="password"');expect(html).toContain('API key');expect(html).toContain('Verify &amp; connect');expect(html).not.toContain('Sign in with');expect(isTrackedProvider(provider)).toBe(true);});
 it.each(['antigravity','devin']as const)('%s existing native readers request no pasted credentials',provider=>{const html=renderToStaticMarkup(<ProviderConnection {...props} provider={provider}/>);expect(html).toContain(`Use existing ${provider==='devin'?'Devin':'Antigravity'}`);expect(html).not.toContain('<input');expect(html).not.toContain('Verify &amp; connect');expect(isTrackedProvider(provider)).toBe(true);});
 it('personal Cursor retains source and honest missing allowance',()=>{const a:AccountState={account:{id:'cursor-fixture',provider:'cursor',email:'personal@example.test',label:'Personal',sources:[]},status:'ready',fault:null,report:{plan:'Pro',source:'cursor_dashboard',observedAt:new Date().toISOString(),quotas:[]}};const html=renderToStaticMarkup(<ProviderPanel selected="cursor" snapshot={{...emptySnapshot,accounts:[a]}} settings={defaultSettings} now={Date.now()} example={false} savingOrder={false} reorder={async()=>{}} manage={()=>{}}/>);expect(html).toContain('unofficial personal dashboard');expect(html).toContain('did not report a numeric individual allowance');expect(html).not.toContain('role="progressbar"');expect(html).not.toContain('token-summary');});
 it.each(['cursor','copilot']as const)('%s account values never borrow local token history',provider=>{const a:AccountState={account:{id:'api-fixture',provider,email:null,label:'fixture-user',sources:[]},status:'ready',fault:null,report:{plan:'Reported usage',observedAt:new Date().toISOString(),quotas:[{id:'reported',label:'Reported usage',percent:null,value:42,unit:'requests',resetsAt:null}]}};const html=renderToStaticMarkup(<ProviderPanel selected={provider} snapshot={{...emptySnapshot,accounts:[a]}} settings={defaultSettings} now={Date.now()} example={false} savingOrder={false} reorder={async()=>{}} manage={()=>{}}/>);expect(html).toContain('42 requests');expect(html).not.toContain('Claude account');expect(html).not.toContain('token-summary');expect(isHistoryProvider(provider)).toBe(false);});
  it('opens sign-in for a provider whose saved login expired, and guides providers without one',()=>{
    const expired=(provider:AccountState['account']['provider']):AccountState=>({account:{id:provider,provider,label:provider,email:null,sources:[]},status:'faulted',report:null,fault:{kind:'needs_relogin',message:'expired',retryAfterSecs:null}});
    const cursor=renderToStaticMarkup(<ProviderConnection {...props} provider="cursor" accounts={[expired('cursor')]}/>);
    expect(cursor).toContain('1 Cursor account needs you to sign in again.');
    expect(cursor).toContain('Hide sign-in options');
    expect(cursor).toContain('Sign in with');
    const devin=renderToStaticMarkup(<ProviderConnection {...props} provider="devin" accounts={[expired('devin'),expired('devin')]}/>);
    expect(devin).toContain('2 Devin accounts need you to sign in again.');
    expect(devin).toContain('official Devin app or CLI');
    const fresh=renderToStaticMarkup(<ProviderConnection {...props} provider="muse"/>);
    expect(fresh).toContain('Not signed in yet?');
  });
  it('explains a Grok account that reports only a balance, and that Cursor sign-in happens in the Cursor app',()=>{
    const grok:AccountState={account:{id:'grok-fixture',provider:'grok',email:'school@example.test',label:'Grok',sources:[]},status:'ready',fault:null,report:{plan:null,source:'grok_cli',observedAt:new Date().toISOString(),quotas:[{id:'grok_prepaid_balance',label:'Prepaid balance · USD',percent:null,value:0,unit:'USD',resetsAt:null}]}};
    const html=renderToStaticMarkup(<ProviderPanel selected="grok" snapshot={{...emptySnapshot,accounts:[grok]}} settings={defaultSettings} now={Date.now()} example={false} savingOrder={false} reorder={async()=>{}} manage={()=>{}}/>);
    expect(html).toContain('no subscription plan or usage meter for school@example.test');
    const subscribed=renderToStaticMarkup(<ProviderPanel selected="grok" snapshot={{...emptySnapshot,accounts:[{...grok,report:{...grok.report!,plan:'SuperGrok'}}]}} settings={defaultSettings} now={Date.now()} example={false} savingOrder={false} reorder={async()=>{}} manage={()=>{}}/>);
    expect(subscribed).not.toContain('no subscription plan');
    const expired:AccountState={account:{id:'c',provider:'cursor',label:'c',email:null,sources:[]},status:'faulted',report:null,fault:{kind:'needs_relogin',message:'expired',retryAfterSecs:null}};
    expect(renderToStaticMarkup(<ProviderConnection {...props} provider="cursor" accounts={[expired]}/>)).toContain('This opens the Cursor app.');
  });
  it('offers an Add account button once a provider has a working account, opening that provider’s sign-in',()=>{
    const ready=(provider:AccountState['account']['provider']):AccountState=>({account:{id:provider,provider,label:provider,email:'me@example.test',sources:[]},status:'ready',fault:null,report:{plan:null,observedAt:new Date().toISOString(),quotas:[{id:'session',label:'Session',percent:10,value:null,unit:null,resetsAt:null}]}});
    const panel=(provider:'codex'|'grok')=>renderToStaticMarkup(<ProviderPanel selected={provider} snapshot={{...emptySnapshot,accounts:[ready(provider)]}} settings={defaultSettings} now={Date.now()} example={false} savingOrder={false} reorder={async()=>{}} manage={()=>{}} connection={(adding:boolean)=><ProviderConnection {...props} provider={provider} adding={adding}/>}/>);
    expect(panel('codex')).toContain('Add another Codex account');
    expect(panel('grok')).toContain('Add another Grok account');
    expect(renderToStaticMarkup(<ProviderPanel selected="codex" snapshot={emptySnapshot} settings={defaultSettings} now={Date.now()} example={false} savingOrder={false} reorder={async()=>{}} manage={()=>{}} connection={(adding:boolean)=><ProviderConnection {...props} provider="codex" adding={adding}/>}/>)).not.toContain('Add another Codex account');
    expect(renderToStaticMarkup(<ProviderConnection {...props} provider="claude" adding/>)).toContain('Sign in with Claude');
    expect(renderToStaticMarkup(<ProviderConnection {...props} provider="grok" adding/>)).toContain('Sign in another Grok account');
    expect(renderToStaticMarkup(<ProviderConnection {...props} provider="deepseek" adding/>)).toContain('<input');
    const devin=renderToStaticMarkup(<ProviderConnection {...props} provider="devin" adding/>);
    expect(devin).toContain('tracks one account at a time');
    expect(devin).toContain('Use existing Devin');
  });
});
