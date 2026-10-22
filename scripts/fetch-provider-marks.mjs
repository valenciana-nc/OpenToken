// Maintainer-only: pinned provider marks matching the reference grid; no runtime fetches.
import { mkdir, writeFile } from 'node:fs/promises';
const revision = '74f64b62b5d38b9cea59107d15799d69563eb38a';
const root = `https://raw.githubusercontent.com/steipete/CodexBar/${revision}`;
const ids = ['codex', 'claude', 'antigravity', 'grok', 'muse', 'cursor', 'copilot', 'devin', 'zai', 'deepseek', 'kimi', 'opencode'];
const dir = new URL('../src/assets/providers/', import.meta.url);
await mkdir(dir, { recursive: true });
for (const id of ids) {
  const response = await fetch(`${root}/Sources/CodexBar/Resources/ProviderIcon-${id}.svg`);
  if (!response.ok) throw Error(`${id}: ${response.status}`);
  const svg = await response.text();
  if (!svg.includes('<svg') || /<script|<foreignObject|<image|\bon\w+=|(?:href|url)\s*[=(]\s*["']?https?:/i.test(svg)) throw Error(`Unsafe or unexpected SVG: ${id}`);
  await writeFile(new URL(`${id}.svg`, dir), svg);
}
await writeFile(new URL('NOTICE.md', dir), `# Provider marks\n\nBundled from CodexBar, revision ${revision}.\nSource: ${root}/Sources/CodexBar/Resources/\n\nMIT License, Copyright (c) 2026 Peter Steinberger.\nFull permission/disclaimer: docs/THIRD-PARTY-LICENSES.txt.\nProvider marks remain their owners' trademarks. No runtime downloads.\n`);
console.log(`Bundled ${ids.length} provider marks from ${revision}.`);
