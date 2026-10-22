// Maintainer-only reference-rate snapshot. The app never downloads prices at runtime.
import { writeFile } from 'node:fs/promises';
const repo = 'BerriAI/litellm';
const commit = await fetch(`https://api.github.com/repos/${repo}/commits/main`).then(r => { if (!r.ok) throw Error(`GitHub: ${r.status}`); return r.json(); });
const source = `https://raw.githubusercontent.com/${repo}/${commit.sha}/model_prices_and_context_window.json`;
const data = await fetch(source).then(r => { if (!r.ok) throw Error(`Prices: ${r.status}`); return r.json(); });
const models = {};
for (const [name, entry] of Object.entries(data)) {
  if (!/^(gpt-|claude-|codex-|o[134](?:-|$))/.test(name) || !['openai', 'anthropic'].includes(entry.litellm_provider)) continue;
  const rates = Object.fromEntries(Object.entries(entry).filter(([k, v]) => /^(input_cost_per_token|output_cost_per_token|cache_read_input_token_cost|cache_creation_input_token_cost)/.test(k) && typeof v === 'number' && Number.isFinite(v) && v >= 0));
  if (rates.input_cost_per_token == null || rates.output_cost_per_token == null) continue;
  models[name] = rates;
}
await writeFile(new URL('../crates/opentoken-core/src/spending/prices.json', import.meta.url), JSON.stringify({ date: new Date().toISOString().slice(0, 10), source, models }, null, 2) + '\n');
console.log(`Bundled ${Object.keys(models).length} reference model rates from ${commit.sha}.`);
