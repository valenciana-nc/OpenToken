import { useState } from 'react';
import { providerDefinition } from '../providers';
import { Info, Share } from 'lucide-react';
import type { AccountState, Provider, Spending, SpendPeriod } from '../types';

type Period = 'Today' | 'Yesterday' | '30 Days';
interface Spend { name: string; amount: number | null; color: string }
const colors: Record<string, string> = { Cursor: '#292c30', Claude: '#dc795b', Codex: '#21a887', Grok: '#909194', 'Muse Code':'#1685ee', Kimi:'#529b72' };
const examples: Record<Period, Record<string, number>> = {
  Today: { Cursor: 66.12, Claude: 49.85, Codex: 1.74, Grok: 0.62 },
  Yesterday: { Cursor: 91.10, Claude: 1.31, Codex: 2.26, Grok: 0.40 },
  '30 Days': { Cursor: 7121.12, Claude: 6042.64, Codex: 134.46, Grok: 71.02 },
};
const exampleTokens: Record<Period, Record<string, number>> = {
  Today: { Codex: 1240000, Claude: 35800000 },
  Yesterday: { Codex: 1610000, Claude: 73800 },
  '30 Days': { Codex: 95820000, Claude: 4340000000 },
};
export function providerPeriod(period: SpendPeriod | undefined, provider?: Provider): SpendPeriod | undefined {
  if (!period || !provider) return period;
  const p = period.providers.find(p => p.provider === provider);
  return { tokens: p?.tokens ?? 0, estimatedUsd: p?.estimatedUsd ?? null, unpricedTokens: p?.unpricedTokens ?? 0, providers: p ? [p] : [], creditChanges: period.creditChanges.filter(c => c.provider === provider) };
}
export const dollars = (n: number) => new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(n);
export const compactTokens = (n: number) => new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 }).format(n);

export default function SpendOverview({ accounts, provider, example, history, historyError, pricing, copy }: { accounts: AccountState[]; provider?: Provider; example: boolean; history: Spending | null; historyError: string; pricing: () => void; copy: () => void }) {
  const [period, setPeriod] = useState<Period>('30 Days');
  const data = providerPeriod(history?.periods[period], provider);
  const providerName = provider ? providerDefinition(provider).name : null;
  const summary = (p: Period) => {
    const scoped = providerPeriod(history?.periods[p], provider);
    const tokens = example ? Object.entries(exampleTokens[p]).filter(([name]) => !providerName || name === providerName).reduce((n, [, value]) => n + value, 0) : scoped?.tokens ?? 0;
    const value = example ? Object.entries(examples[p]).filter(([name]) => !providerName || name === providerName).reduce((n, [, value]) => n + value, 0) : scoped?.estimatedUsd;
    return { tokens, value, partial: !example && !!scoped?.unpricedTokens };
  };
  const tokenBasis = !example && !!data?.tokens && data.unpricedTokens > 0 && !data.estimatedUsd;
  const partialEstimate = !example && !tokenBasis && !!data?.unpricedTokens;
  const names = [...new Set([...accounts.filter(a => !provider || a.account.provider === provider).map(a => providerDefinition(a.account.provider).name), ...(data?.providers.map(p => providerDefinition(p.provider).name) ?? [])])];
  const rows: Spend[] = example
    ? Object.entries(examples[period]).filter(([name]) => !providerName || name === providerName).map(([name, amount]) => ({ name, amount, color: colors[name] }))
    : names.map(name => { const p = data?.providers.find(p => providerDefinition(p.provider).name === name); return { name, amount: p ? tokenBasis ? p.tokens : p.estimatedUsd : null, color: colors[name] }; });
  const total = example ? rows.reduce((sum, row) => sum + (row.amount ?? 0), 0) : tokenBasis ? data!.tokens : data?.estimatedUsd ?? null;
  const circumference = 2 * Math.PI * 67;
  let offset = 0;
  const range = period === '30 Days' ? 'the last 30 days' : period.toLowerCase();
  const display = (value: number) => tokenBasis ? compactTokens(value) : dollars(value);
  const explanation = 'Supported local CLI records supply measured token counts for this device, independently of account login. Cached/reasoning subsets are not counted twice. Dollar values use reference API rates, including known cache and long-context rates. They are estimates of usage value—not subscription charges or invoices. Unknown model rates remain unavailable, never as zero. Credit history records observed balance changes, not verified purchases.';
  return <section className="spend-overview" aria-labelledby="spend-heading">
    <div className="token-summary" role="group" aria-label="Recorded token usage">
      {(['Today', '30 Days'] as const).map(p => { const s = summary(p); return <div className="token-stat" key={p}><span>{p === '30 Days' ? 'Last 30 days' : p}</span><strong>{s.tokens > 0 ? compactTokens(s.tokens) : '—'} tokens</strong>{s.value != null && (s.value > 0 || !s.partial) ? <small>{s.partial ? '≥' : ''}{dollars(s.value)} <span>estimated value</span></small> : s.tokens > 0 && s.partial ? <small>Prices incomplete</small> : null}</div>; })}
    </div>
    <div className="spend-heading">
      <div className="spend-title"><h2 id="spend-heading">Total Spend</h2>
        <details className="spend-help"><summary aria-label="About spending data"><Info size={14} aria-hidden="true"/></summary><p>{explanation}</p></details>
      </div>
      <button className="icon-button" aria-label="Copy usage report" title="Copy your usage report, without credential-folder paths" onClick={copy}><Share size={18}/></button>
    </div>
    <div className="spend-surface">
      <div className="period-control" role="group" aria-label="Spending period">
        {(['Today', 'Yesterday', '30 Days'] as const).map(p => <button key={p} aria-pressed={p === period} onClick={() => setPeriod(p)}>{p}</button>)}
      </div>
      <div className="spend-breakdown">
        <div className={`donut ${total == null ? 'unavailable' : ''}`} role="img" aria-label={total == null ? `Local spending data unavailable for ${range}.` : example ? `${period} sample spending: ${dollars(total)}` : `${tokenBasis ? 'Recorded tokens' : `${partialEstimate ? 'At least ' : ''}estimated API-equivalent usage value`} for ${range}: ${display(total)}`}>
          <svg viewBox="0 0 180 180" aria-hidden="true">
            <circle className="donut-track" cx="90" cy="90" r="67" fill="none" strokeWidth="27"/>
            {total != null && total > 0 && rows.map(row => {
              const fraction = (row.amount ?? 0) / total;
              const dash = Math.max(0, fraction * circumference - 3);
              const start = offset; offset += fraction * circumference;
              return <circle key={row.name} cx="90" cy="90" r="67" fill="none" stroke={row.color} strokeWidth="27" strokeDasharray={`${dash} ${circumference - dash}`} strokeDashoffset={-start} transform="rotate(-90 90 90)"/>;
            })}
          </svg>
          <div className="donut-total"><strong>{partialEstimate ? '≥' : ''}{total == null ? '—' : tokenBasis ? compactTokens(total) : total >= 1000 ? `$${(total / 1000).toFixed(1)}K` : dollars(total)}</strong>{!example && <small>{total == null ? 'Not recorded' : tokenBasis ? 'tokens · unpriced' : 'API-equivalent · estimate'}</small>}</div>
        </div>
        <div className="spend-legend-column">{!rows.length && <span className="spend-empty-legend">No connected accounts</span>}<dl className="spend-legend">{rows.map(row => <div key={row.name}><dt><i style={{ backgroundColor: row.color }}/>{row.name}</dt><dd>{row.amount == null ? '—' : display(row.amount)}</dd></div>)}</dl>{!example && rows.length > 0 && <span className="spend-source-note">This device · {providerName??'CLI'} client records{data?.tokens ? <><br/>{compactTokens(data.tokens)} tokens</> : null}{history?.indexing ? <><br/>Indexing history… {history.filesPending} files left</> : null}</span>}</div>
      </div>
      {!example && <div className="spend-unavailable" aria-live="polite">
        {historyError ? <p>Local history for {range} is unavailable. {historyError}</p> : total == null ? <p>{history?.indexing ? 'Reading local usage history…' : `No local token usage recorded for ${range}.`} Billed charges aren’t shared by subscription quota APIs.</p> : <p>{tokenBasis ? 'Some models have no reference rate. The ring shows recorded tokens instead.' : 'Estimated token value, not money charged to your subscription.'}{history?.indexing ? ' Totals are partial while history is indexed.' : ''}</p>}
        {partialEstimate && <p>Lower bound: {compactTokens(data!.unpricedTokens)} tokens have no rate. Unpriced models across indexed sources: {history?.unpricedModels.join(', ')}.</p>}
        {data?.tokens ? <button className="pricing-link" onClick={pricing}>{data.unpricedTokens > 0 ? 'Set missing model rates' : 'Review reference rates'}</button> : null}
        {history && history.skippedRecords > 0 && <p>{history.skippedRecords} malformed or oversized records skipped; totals may be incomplete.</p>}
        {history?.creditTrackingSince && <details className="credit-history"><summary>Observed credit changes</summary><p>Credit observation across connected accounts started {new Date(history.creditTrackingSince).toLocaleDateString()}. Balance decreases can include expiry; they aren’t verified billed spending.</p>{data?.creditChanges.length ? data.creditChanges.map(c => <p key={`${c.provider}-${c.unit}`}>{providerDefinition(c.provider).name}: −{c.decrease.toLocaleString(undefined, { maximumFractionDigits: 2 })} / +{c.increase.toLocaleString(undefined, { maximumFractionDigits: 2 })} {c.unit}</p>) : <p>No observed changes for {range}.</p>}</details>}
      </div>}
    </div>
  </section>;
}
