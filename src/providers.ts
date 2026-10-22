import type { Provider } from './types';
import codex from './assets/providers/codex.svg';
import claude from './assets/providers/claude.svg';
import cursor from './assets/providers/cursor.svg';
import antigravity from './assets/providers/antigravity.svg';
import copilot from './assets/providers/copilot.svg';
import grok from './assets/providers/grok.svg';
import muse from './assets/providers/muse.svg';
import devin from './assets/providers/devin.svg';
import zai from './assets/providers/zai.svg';
import deepseek from './assets/providers/deepseek.svg';
import kimi from './assets/providers/kimi.svg';
import opencode from './assets/providers/opencode.svg';

export const providers = [
  { id: 'codex', name: 'Codex', mark: codex, accent: '#24a9b9' },
  { id: 'claude', name: 'Claude', mark: claude, accent: '#cf856b' },
  { id: 'antigravity', name: 'Antigravity', mark: antigravity, accent: '#e04c4c' },
  { id: 'grok', name: 'Grok', mark: grok, accent: '#2aa781' },
  { id: 'muse', name: 'Muse Code', mark: muse, accent: '#1685ee' },
  { id: 'cursor', name: 'Cursor', mark: cursor, accent: '#e9943c' },
  { id: 'copilot', name: 'Copilot', mark: copilot, accent: '#ac68e8' },
  { id: 'devin', name: 'Devin', mark: devin, accent: '#5b83e8' },
  { id: 'zai', name: 'Z.ai', mark: zai, accent: '#99999c' },
  { id: 'deepseek', name: 'DeepSeek', mark: deepseek, accent: '#4d6bfe' },
  { id: 'kimi', name: 'Kimi', mark: kimi, accent: '#529b72' },
  { id: 'opencode', name: 'OpenCode', mark: opencode, accent: '#99999c' },
] as const;
export type ProviderId = typeof providers[number]['id'];
export function isTrackedProvider(id: ProviderId): id is Provider { return providers.some(p => p.id === id); }
export function isHistoryProvider(id: ProviderId): id is 'codex' | 'claude' | 'muse' | 'grok' | 'kimi' { return ['codex','claude','muse','grok','kimi'].includes(id); }
export function connectionScope(provider: ProviderId, personalCursor = false, githubQuota = false, museKey = false) {
  if (provider === 'cursor' && personalCursor) return 'Cursor app sign-in · unofficial personal dashboard. Only provider-reported monthly/Auto/API allowances and reset dates are shown. No Enterprise key, local token estimates or invoices; dashboard access may change.';
  if (provider === 'cursor') return 'Cursor Enterprise admin API · selected team member only. On-demand budget is not the included subscription allowance. Reported usage value is not an invoice; personal Cursor plans are not supported by this API.';
  if (provider === 'copilot' && githubQuota) return 'GitHub CLI sign-in · unofficial Copilot entitlement API. Only reported metered allowances/resets; unlimited or missing quota is unavailable, not 100% left. GitHub CLI retains credentials. This is not personal billing or an invoice.';
  if (provider === 'copilot') return 'GitHub personal billing API · current UTC calendar month. Premium requests and AI credits are reported separately, not added together. Net usage amounts are not invoices. Organization-paid usage and remaining subscription allowances are not provided; an empty report is not zero usage.';
  if (provider === 'devin') return 'Existing official Devin CLI or desktop sign-in · unofficial status RPC. Only explicit daily/weekly percentages drive remaining allowance. Hidden daily fields with unverified window meaning stay raw values, never a fabricated weekly bar. Extra balance is separate; no browser extraction or organization ACU attribution.';
  if (provider === 'antigravity') return 'Owned Antigravity app/IDE service, or explicitly reused valid agy CLI token from its exact Windows credential entry. Identity and source are checked around the read. CLI uses the first-party daily quota host only; production Gemini is a different pool and is never substituted. Missing buckets remain unknown. No browser data, Google token renewal or copied credentials.';
  if (provider === 'muse' && museKey) return 'Explicitly authorized Meta CLI key endpoint · NOT read-only. Each background refresh may issue or rotate an inference key and replacement effects are undocumented. Monitoring continues until disconnected; returned inference keys are discarded, never used or written into Muse. Only authenticated subscription percentages/reset dates are shown; no prompts, payments, browser extraction or borrowed OAuth registration. A request already sent cannot be undone.';
  if (provider === 'muse') return 'Official Muse Session Protocol · last-observed subscription windows, not a guaranteed fresh account query. A new host may have no reading; no prompt, session creation or key-mint fallback is requested. Local recorded tokens are independent. Profile credentials remain with Muse. Advanced subscription refresh is a separate opt-in credential-issuing endpoint: native confirmation is required, background requests repeat until disconnected, returned inference keys are discarded, and provider-side effects cannot be rolled back.';
  if (provider === 'grok') return 'Official Grok CLI sign-in · private authenticated credit/budget API. Grok owns credentials. Identity is checked around the read; missing usage is unknown, not zero. This is not an xAI management-key or team-usage integration.';
  if (provider === 'zai') return 'Z.ai Coding Plan quota API · the selected Global or China region and API-key scope. Only reported limits/resets are shown; no invented account email or prices.';
  if (provider === 'kimi') return 'Kimi Code subscription quota · the selected China or International region. Not Moonshot/Open Platform credits. Only reported windows and reset dates.';
  if (provider === 'deepseek') return 'DeepSeek official API balance · paid and granted credit by currency. Not DeepSeek Chat usage, subscription percent-left, tokens or an invoice.';
  if (provider === 'opencode') return 'OpenCode Go API-key quota · not every OpenCode client session or provider account. Only reported rolling/weekly/monthly percentages; no local token estimates.';
  return '';
}
export function providerDefinition(id: ProviderId) { return providers.find(p => p.id === id)!; }
