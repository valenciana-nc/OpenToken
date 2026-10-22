import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { Spending, SpendPeriod } from '../types';
import SpendOverview, { providerPeriod } from './SpendOverview';
function render(period: SpendPeriod) {
  const history: Spending = { updatedAt: new Date().toISOString(), indexing: false, filesIndexed: 1, filesPending: 0, skippedRecords: 0, pricingDate: '2026-10-14', referencePrices: {}, creditTrackingSince: null, unpricedModels: ['unpriced-model'], periods: { Today: period, Yesterday: period, '30 Days': period } };
  return renderToStaticMarkup(<SpendOverview accounts={[]} example={false} history={history} historyError="" pricing={() => {}} copy={() => {}}/>);
}
describe('provider-scoped local history', () => {
  const period: SpendPeriod = { tokens: 300, estimatedUsd: 30, unpricedTokens: 50, providers: [{ provider: 'codex', tokens: 100, estimatedUsd: 10, unpricedTokens: 50 }, { provider: 'claude', tokens: 200, estimatedUsd: 20, unpricedTokens: 0 }], creditChanges: [{ provider: 'codex', unit: 'credits', decrease: 7, increase: 0 }] };
  it('isolates provider tokens, value, unknown prices and observed balances without mutating history', () => {
    const before = JSON.stringify(period);
    expect(providerPeriod(period, 'claude')).toEqual({ tokens: 200, estimatedUsd: 20, unpricedTokens: 0, providers: [period.providers[1]], creditChanges: [] });
    expect(providerPeriod(period, 'codex')?.creditChanges[0].decrease).toBe(7);
    expect(JSON.stringify(period)).toBe(before);
  });
  it('never borrows another provider’s history for an unrecorded source', () => {
    expect(providerPeriod({ ...period, providers: [period.providers[0]] }, 'claude')).toEqual({ tokens: 0, estimatedUsd: null, unpricedTokens: 0, providers: [], creditChanges: [] });
    expect(providerPeriod(undefined, 'claude')).toBeUndefined(); expect(providerPeriod(period)).toBe(period);
  });
});
describe('live usage-value ring', () => {
  it('shows a lower-bound dollar value when only part of usage is priced', () => {
    const html = render({ tokens: 10000, estimatedUsd: 12.5, unpricedTokens: 1000, providers: [{ provider: 'codex', tokens: 10000, estimatedUsd: 12.5, unpricedTokens: 1000 }], creditChanges: [] });
    expect(html).toContain('≥$12.50');
    expect(html).toContain('Lower bound:');
    expect(html).toContain('not money charged');
    expect(html).toContain('unpriced-model');
  });
  it('shows tokens, not invented zero-dollar spending, when all rates are unknown', () => {
    const html = render({ tokens: 10000, estimatedUsd: null, unpricedTokens: 10000, providers: [{ provider: 'codex', tokens: 10000, estimatedUsd: null, unpricedTokens: 10000 }], creditChanges: [] });
    expect(html).toContain('10K'); expect(html).toContain('tokens · unpriced'); expect(html).not.toContain('$0.00');
  });
  it('keeps the full ring and periods when no local usage exists', () => {
    const html = render({ tokens: 0, estimatedUsd: null, unpricedTokens: 0, providers: [], creditChanges: [] });
    expect(html).toContain('Not recorded'); expect(html).toContain('donut-track'); expect(html).toContain('Yesterday'); expect(html).not.toContain('$0.00');
  });
});
