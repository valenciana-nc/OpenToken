import { useId, useState } from 'react';
import { ChevronDown, ChevronUp, GripVertical, Info } from 'lucide-react';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { AccountState } from '../types';
import { amount, countdown, remaining } from '../format';
import ProviderGlyph from './ProviderGlyph';
import { providerDefinition } from '../providers';

const DETAIL_METRICS = new Set(['credits', 'resets']);

export default function AccountSection({ state: { account, report, status, fault }, now, example = false, reorderDisabled = false, reorderSaving = false, heading = true }: { state: AccountState; now: number; example?: boolean; reorderDisabled?: boolean; reorderSaving?: boolean; heading?: boolean }) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({ id: account.id, disabled: reorderDisabled || reorderSaving });
  const [expanded, setExpanded] = useState(false);
  const detailsId = useId();
  const usageQuotas = report?.quotas.filter(q => !DETAIL_METRICS.has(q.id)) ?? [];
  const detailQuotas = report?.quotas.filter(q => DETAIL_METRICS.has(q.id)) ?? [];
  const provider = providerDefinition(account.provider).name;
  const customName = !account.label.startsWith('Codex ·') && account.label !== provider && account.label !== account.email ? account.label : null;
  const today = example && account.provider === 'claude' ? '$49.85 · 35.8M tokens' : null;
  const yesterday = example && account.provider === 'claude' ? '$1.31 · 73.8k tokens' : null;
  return <article ref={setNodeRef} className={`account-card ${status}${isDragging ? ' dragging' : ''}`} style={{ transform: CSS.Translate.toString(transform), transition }} id={account.id} aria-label={`${provider} account`}>
    {heading && <header className="account-heading"><button ref={setActivatorNodeRef} className="account-drag-handle" {...attributes} {...listeners} disabled={reorderDisabled} aria-label={`Reorder ${provider}${account.label !== provider ? ` ${account.label}` : ''}`} title={reorderSaving ? 'Saving account order…' : reorderDisabled ? 'Connect another account to reorder cards' : 'Drag to reorder · or press Space, then Up/Down'}><GripVertical size={17} aria-hidden="true"/></button><div className="account-title"><div><h2>{customName || account.email || provider}</h2>{report?.plan && <span className="plan">{report.plan}</span>}{status === 'stale' && <span className="cached-label">Cached</span>}{report?.source === 'cursor_dashboard' && <span className="cached-label" title="Read through Cursor’s existing app session; unofficial dashboard API">Cursor app</span>}{report?.source === 'cli_log' && <span className="cached-label" title="Explicit log identity matched; quota awaits provider confirmation">CLI log</span>}</div>{customName && account.email && <p>{account.email}</p>}</div><span className="provider-logo"><ProviderGlyph provider={account.provider}/></span></header>}
    <div className="metrics-surface">
      {fault && <div className="card-fault"><Info size={15}/>{fault.message}</div>}
      <div className="quota-list">{usageQuotas.map(q => {
        const left = remaining(q.percent);
        const projection = q.projectedRemainingAtReset;
        return left == null ? <div className="value-row" key={q.id}><span>{q.label}</span><strong>{amount(q)}</strong></div> : <div className="quota" key={q.id}>
          <div className="quota-title"><h3>{q.label} <strong>{left}% left</strong></h3>{q.resetsAt && <span className="reset">{countdown(q.resetsAt, now)}</span>}</div>
          <div className="track" role="progressbar" aria-label={`${q.label} remaining`} aria-valuenow={left} aria-valuemin={0} aria-valuemax={100} aria-valuetext={`${left}% left`}><div style={{ width: `${left}%` }}/>{projection != null && <i className="forecast-tick" style={{ left: `${Math.max(0, Math.min(100, projection))}%` }} title={`Projected ${projection}% left at reset`}/>}</div>
          {(projection != null || q.value != null) && <div className="quota-caption">{projection != null && <span>~{projection}% left at reset</span>}{q.value != null && <span>{amount(q)}</span>}</div>}
        </div>;
      })}</div>
      <button className="details-toggle" aria-label={expanded ? `Collapse ${provider} details` : `Expand ${provider} details`} aria-expanded={expanded} aria-controls={detailsId} onClick={() => setExpanded(!expanded)}><span>Usage details</span>{expanded ? <ChevronUp size={20}/> : <ChevronDown size={20}/>}</button>
      <div className="account-details" id={detailsId} hidden={!expanded}>{status === 'stale' && <p className="details-note">Cached reading · not live. Automatic retries are active; this account is still monitored.</p>}<p className="details-note">Quota source: {!report ? 'Unavailable' : report.source === 'cursor_dashboard' ? `${status === 'stale' ? 'Cached · ' : ''}Cursor app · unofficial dashboard` : report.source === 'devin_app' ? 'Devin app · unofficial quota RPC · credential scope' : report.source === 'devin_cli' ? 'Devin CLI · unofficial quota RPC · credential scope' : report.source === 'antigravity_cli_usage' ? 'Official agy /usage · current CLI profile · account identity not provided' : report.source === 'antigravity_stored' ? 'Antigravity IDE · stored session · daily Cloud Code quota' : report.source === 'antigravity_cli' ? 'Antigravity CLI · daily Cloud Code quota' : report.source === 'antigravity_local' ? 'Antigravity app · owned local quota RPC' : report.source === 'muse_device' ? 'Muse · authorized key-issuing quota endpoint' : report.source === 'muse_cli' ? 'Muse CLI · last-observed subscription · as of' : report.source === 'grok_cli' ? 'Grok CLI · private credit API' : report.source === 'copilot_internal' ? 'GitHub CLI · unofficial Copilot entitlement API' : report.source === 'cli_log' ? 'CLI log · identity matched; awaiting provider confirmation' : status === 'stale' ? 'Cached provider reading' : 'Provider verified'}{report && <> · {new Date(report.source==='muse_cli'?report.observedAt:report.verifiedAt ?? report.observedAt).toLocaleTimeString()}</>}</p><dl>{detailQuotas.map(q => <div key={q.id}><dt>{q.label}</dt><dd>{amount(q)}</dd></div>)}<div><dt>Today</dt><dd>{today ?? <span className="not-reported">Not reported</span>}</dd></div><div><dt>Yesterday</dt><dd>{yesterday ?? <span className="not-reported">Not reported</span>}</dd></div></dl>{!today && <p className="details-note">{report?.source === 'cursor_dashboard' ? 'Unofficial dashboard reading through your existing Cursor sign-in, not an invoice. No local token counts or costs are inferred.' : account.provider !== 'codex' && account.provider !== 'claude' ? 'Provider usage or balance report, not an invoice. This connection does not import local token history or infer subscription allowances.' : 'Billed costs and token counts aren’t included in subscription quota readings.'}</p>}</div>
    </div>
  </article>;
}
