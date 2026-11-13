import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { defaultSettings, emptySnapshot, type AccountState } from '../types';
import { isTrackedProvider, providerDefinition, providers } from '../providers';
import ProviderGrid, { providerRemaining } from './ProviderGrid';
import ProviderPanel from './ProviderPanel';
function account(percent: number | null, status: AccountState['status'] = 'ready'): AccountState {
  return { account: { id: String(percent), provider: 'codex', label: 'Test', email: null, sources: [] }, status, fault: null, report: { plan: null, observedAt: new Date().toISOString(), quotas: [{ id: 'session', label: 'Session', percent, resetsAt: null, value: null, unit: null }] } };
}
describe('reference provider picker', () => {
  it('bundles every requested distinct mark in the requested priority order, with only real connectors enabled', () => {
    expect(providers.map(p => p.name)).toEqual(['Codex', 'Claude', 'Antigravity', 'Grok', 'Muse Code', 'Cursor', 'Copilot', 'Devin', 'Z.ai', 'DeepSeek', 'Kimi', 'OpenCode']);
    expect(new Set(providers.map(p => p.mark)).size).toBe(12);
    expect(providers.filter(p => isTrackedProvider(p.id)).map(p => p.id)).toEqual(['codex', 'claude', 'antigravity', 'grok', 'muse', 'cursor', 'copilot', 'devin', 'zai', 'deepseek', 'kimi', 'opencode']);
  });
  it('renders twelve accessible tabs and the separate Add action, never Overview', () => {
    const html = renderToStaticMarkup(<ProviderGrid selected="codex" select={() => {}} accounts={[]} add={() => {}}/>);
    expect(html.match(/role="tab"/g)).toHaveLength(12);
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
    expect(html).toContain('aria-label="Add provider"'); expect(html).not.toContain('Overview');
  });
  it('keeps the bare Add glyph decorative and separate from the provider tabs', () => {
    const html = renderToStaticMarkup(<ProviderGrid selected="codex" select={() => {}} accounts={[]} add={() => {}}/>);
    expect(html).toContain('class="add-provider-mark" aria-hidden="true"');
    expect(html).toMatch(/class="add-provider-mark"[^>]*><svg[^>]*width="31"[^>]*height="31"[^>]*stroke-width="2.5"/);
    expect(html).toContain('class="provider-tab provider-add" aria-label="Add provider"');
    expect(html.match(/role="tab"/g)).toHaveLength(12);
  });
  it.each(['antigravity', 'devin'] as const)('keeps disconnected %s separate and never borrows another account’s usage', selected => {
    expect(isTrackedProvider(selected)).toBe(true);
    expect(providerRemaining([account(20)], selected)).toBeNull();
    const html = renderToStaticMarkup(<ProviderPanel selected={selected} snapshot={{ ...emptySnapshot, accounts: [account(20)] }} settings={defaultSettings} now={Date.now()} example={false} savingOrder={false} reorder={async () => {}} manage={() => {}}/>);
    expect(html).toContain(`Connect ${providerDefinition(selected).name}`);
    expect(html).not.toContain('role="progressbar"'); expect(html).not.toContain('spend-overview'); expect(html).not.toContain('token-summary');
  });
  it('uses the lowest verified session remaining, not percent used or a guessed aggregate', () => {
    expect(providerRemaining([account(20), account(70)], 'codex')).toBe(30);
    expect(providerRemaining([account(100)], 'codex')).toBe(0);
    expect(providerRemaining([account(0)], 'codex')).toBe(100);
    expect(providerRemaining([account(20)], 'claude')).toBeNull();
    expect(providerRemaining([account(20)], 'muse')).toBeNull();
  });
  it('uses the reported Cursor monthly total instead of an exhausted independent Auto bucket', () => {
    const state = account(25); state.account.provider = 'cursor'; state.report!.source = 'cursor_dashboard';
    state.report!.quotas = [
      { id: 'cursor_monthly', label: 'Monthly plan', percent: 64.27777777777777, resetsAt: null, value: null, unit: null },
      { id: 'cursor_auto', label: 'Auto', percent: 100, resetsAt: null, value: null, unit: null },
      { id: 'cursor_api', label: 'API', percent: 27.266666666666666, resetsAt: null, value: null, unit: null },
    ];
    expect(providerRemaining([state], 'cursor')).toBe(36);
    const html = renderToStaticMarkup(<ProviderGrid selected="cursor" select={() => {}} accounts={[state]} add={() => {}}/>);
    expect(html).toContain('unofficial Cursor dashboard'); expect(html).toContain('Monthly plan: 36% left');
    expect(html).toContain('style="width:36%"');
    state.report!.quotas[0].percent = 100; expect(providerRemaining([state], 'cursor')).toBe(0);
    state.report!.quotas[0].percent = Number.NaN; expect(providerRemaining([state], 'cursor')).toBe(73);
    state.report!.quotas = state.report!.quotas.filter(q => q.id === 'cursor_auto');
    expect(providerRemaining([state], 'cursor')).toBe(0);
    state.report!.quotas = []; expect(providerRemaining([state], 'cursor')).toBeNull();
    state.status = 'stale'; expect(providerRemaining([state], 'cursor')).toBeNull();
  });
  it('keeps Enterprise budget selection and distinct-account aggregation separate from personal Cursor buckets', () => {
    const personal = account(25); personal.account.provider = 'cursor'; personal.report!.source = 'cursor_dashboard';
    personal.report!.quotas[0].id = 'cursor_monthly';
    const enterprise = account(42); enterprise.account.provider = 'cursor';
    enterprise.report!.quotas[0].id = 'cursor_on_demand';
    expect(providerRemaining([personal], 'cursor')).toBe(75);
    expect(providerRemaining([enterprise], 'cursor')).toBe(58);
    expect(providerRemaining([personal, enterprise], 'cursor')).toBe(58);
    enterprise.status = 'stale'; expect(providerRemaining([personal, enterprise], 'cursor')).toBeNull();
  });
  it('uses the shared CSS meter fill for every brand instead of inline provider accents', () => {
    const accounts = providers.map(provider => {
      const state = account(35);
      state.account.provider = provider.id;
      state.account.id = `fixture-${provider.id}`;
      return state;
    });
    const html = renderToStaticMarkup(<ProviderGrid selected="codex" select={() => {}} accounts={accounts} add={() => {}}/>);
    expect(html.match(/class="provider-usage-stripe"/g)).toHaveLength(providers.length);
    expect(html.match(/style="width:65%"/g)).toHaveLength(providers.length);
    expect(html).not.toContain('background-color:');
  });
  it('does not paint fake provider usage when a reading is missing, nonfinite or stale', () => {
    expect(providerRemaining([], 'codex')).toBeNull();
    expect(providerRemaining([account(20), account(null)], 'codex')).toBeNull();
    expect(providerRemaining([account(Number.NaN)], 'codex')).toBeNull();
    expect(providerRemaining([account(20, 'stale')], 'codex')).toBeNull();
  });
  it('shows a working account’s bar even when another sign-in for that provider has expired', () => {
    const expired: AccountState = { ...account(null, 'faulted'), report: null, account: { ...account(null).account, id: 'expired' } };
    expect(providerRemaining([account(20), expired], 'codex')).toBe(80);
    expect(providerRemaining([expired], 'codex')).toBeNull();
  });
});
