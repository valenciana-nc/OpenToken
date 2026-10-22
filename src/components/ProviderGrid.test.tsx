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
  it('uses the most limited actually reported Cursor bucket without averaging or inventing totals', () => {
    const state = account(25); state.account.provider = 'cursor'; state.report!.source = 'cursor_dashboard';
    state.report!.quotas = [
      { id: 'cursor_monthly', label: 'Monthly plan', percent: 25, resetsAt: null, value: null, unit: null },
      { id: 'cursor_api', label: 'API', percent: 42, resetsAt: null, value: null, unit: null },
    ];
    expect(providerRemaining([state], 'cursor')).toBe(58);
    const html = renderToStaticMarkup(<ProviderGrid selected="cursor" select={() => {}} accounts={[state]} add={() => {}}/>);
    expect(html).toContain('unofficial Cursor dashboard'); expect(html).toContain('reported allowance/budget: 58% left');
    state.report!.quotas = []; expect(providerRemaining([state], 'cursor')).toBeNull();
    state.status = 'stale'; expect(providerRemaining([state], 'cursor')).toBeNull();
  });
  it('does not paint fake provider usage when a reading is missing, nonfinite or stale', () => {
    expect(providerRemaining([], 'codex')).toBeNull();
    expect(providerRemaining([account(20), account(null)], 'codex')).toBeNull();
    expect(providerRemaining([account(Number.NaN)], 'codex')).toBeNull();
    expect(providerRemaining([account(20, 'stale')], 'codex')).toBeNull();
  });
});
