import { useState } from 'react';
import { Plus, X } from 'lucide-react';
import type { ModelPrice, Spending } from '../types';
import type { PreferencesProps } from '../ui';
const dollars = (n: number) => new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 0, maximumFractionDigits: 6 }).format(n);
const fields: [keyof ModelPrice, string][] = [['input', 'Input'], ['cached', 'Cached input'], ['output', 'Output'], ['cacheWrite', 'Cache write · 5 min'], ['cacheWriteHour', 'Cache write · 1 hour']];
const blank = () => ({ input: '', cached: '', output: '', cacheWrite: '', cacheWriteHour: '' });
export default function ModelRates({ draft, setDraft, busy, pricing }: Pick<PreferencesProps, 'draft' | 'setDraft' | 'busy'> & { pricing?: Spending | null }) {
  const [model, setModel] = useState('');
  const [rates, setRates] = useState(blank);
  const [error, setError] = useState('');
  const names = [...new Set([...Object.keys(pricing?.referencePrices ?? {}), ...(pricing?.unpricedModels ?? []), ...Object.keys(draft.modelPrices)])].sort();
  function select(name: string) {
    setModel(name); setError('');
    const p = draft.modelPrices[name] ?? pricing?.referencePrices[name];
    setRates(p ? Object.fromEntries(fields.map(([key]) => [key, String(p[key])])) as ReturnType<typeof blank> : blank());
  }
  return <section className="panel model-rates"><h2>Local usage & pricing</h2>
    <p className="muted-text">Codex sessions and Claude project logs supply real token counts, including subscription usage. The ring estimates their API-equivalent value; this isn’t your subscription bill.</p>
    <p className="hint">Bundled LiteLLM reference rates{pricing?.pricingDate ? ` · ${pricing.pricingDate}` : ''}. Cache reads, writes and known long-context tiers are included. Unknown models stay unpriced. Tool fees and unrecorded activity aren’t included.</p>
    <div className="reference-rates">{names.map(name => { const p = draft.modelPrices[name] ?? pricing?.referencePrices[name]; return <div key={name}><b>{name}</b><small>{p ? `Input ${dollars(p.input)} · cached ${dollars(p.cached)} · output ${dollars(p.output)} / million` : 'Unpriced — enter a rate below'}</small>{draft.modelPrices[name] && <button className="icon-button" aria-label={`Remove rate override for ${name}`} disabled={busy} onClick={() => { const next = { ...draft.modelPrices }; delete next[name]; setDraft({ ...draft, modelPrices: next }); }}><X size={13}/></button>}</div>; })}</div>
    <form onSubmit={e => {
      e.preventDefault();
      if (!model.trim() || model.trim().length > 128 || fields.some(([k]) => rates[k].trim() === '' || !Number.isFinite(Number(rates[k])) || Number(rates[k]) < 0 || Number(rates[k]) > 1_000_000)) { setError('Enter a model and all five non-negative USD rates. Zero explicitly means free.'); return; }
      const p = Object.fromEntries(fields.map(([k]) => [k, Number(rates[k])])) as unknown as ModelPrice;
      setDraft({ ...draft, modelPrices: { ...draft.modelPrices, [model.trim().toLowerCase()]: p } });
      setError('');
    }}>
      <label>Model rate override<input list="known-model-rates" placeholder="Exact model name" value={model} maxLength={128} onChange={e => select(e.target.value)}/></label>
      <datalist id="known-model-rates">{names.map(n => <option value={n} key={n}/>)}</datalist>
      <div className="rate-fields">{fields.map(([key, label]) => <label key={key}>{label}<input type="number" min={0} max={1_000_000} step="any" value={rates[key]} placeholder="USD / million" onChange={e => setRates({ ...rates, [key]: e.target.value })}/></label>)}</div>
      {error && <p className="rate-error" role="alert">{error}</p>}
      <button className="text-button" type="submit" disabled={busy}><Plus size={15}/>Use these rates</button>
      <p className="hint">Save preferences to apply. An override replaces tiered reference pricing with these fixed USD-per-million rates. Zero means free, not unknown.</p>
    </form>
  </section>;
}
