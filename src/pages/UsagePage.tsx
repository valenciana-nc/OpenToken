import type { AccountState, Spending } from '../types';
import { providers, type ProviderId } from '../providers';
import SpendOverview from '../components/SpendOverview';

export type UsageFilter = 'all' | ProviderId;

export default function UsagePage({ provider, select, accounts, example, history, historyError, pricing, copy }: {
  provider: UsageFilter; select: (provider: UsageFilter) => void; accounts: AccountState[];
  example: boolean; history: Spending | null; historyError: string; pricing: () => void; copy: () => void;
}) {
  return <div className="usage-page preferences-page">
    <h1>Usage</h1>
    <p className="page-subtitle">Recorded tokens and estimated value on this device—not subscription charges.</p>
    <label>Usage provider<select value={provider} onChange={e => select(e.target.value as UsageFilter)}>
      <option value="all">All providers</option>
      {providers.map(p => <option value={p.id} key={p.id}>{p.name}</option>)}
    </select></label>
    <SpendOverview accounts={accounts} provider={provider === 'all' ? undefined : provider} example={example && (provider === 'all' || provider === 'codex' || provider === 'claude')} history={history} historyError={historyError} pricing={pricing} copy={copy}/>
  </div>;
}
