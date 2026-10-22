// Bundled Simple Icons (Codex/Claude, CC0) and CodexBar (Cursor/Copilot, MIT).
// Rasterized locally; never fetched at runtime. See docs/THIRD-PARTY.md.
import { chromium } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';
const browser = await chromium.launch({ channel: 'msedge' });
try {
  const page = await browser.newPage();
  for (const name of ['openai', 'claude', 'cursor', 'copilot', 'zai', 'deepseek', 'kimi', 'opencode', 'grok', 'antigravity', 'muse', 'devin']) {
    const source = !['openai', 'claude'].includes(name) ? `src/assets/providers/${name}.svg` : `src-tauri/icons/${name}.svg`;
    const svg = await readFile(source, 'utf8');
    const encoded = await page.evaluate(svg => new Promise((resolve, reject) => {
      const image = new Image();
      image.onload = () => {
        const canvas = document.createElement('canvas'); canvas.width = canvas.height = 64;
        canvas.getContext('2d').drawImage(image, 2, 2, 60, 60);
        resolve(canvas.toDataURL('image/png').split(',')[1]);
      };
      image.onerror = reject;
      image.src = `data:image/svg+xml,${encodeURIComponent(svg)}`;
    }), svg);
    await writeFile(`src-tauri/icons/${name}-tray.png`, Buffer.from(encoded, 'base64'));
  }
} finally { await browser.close(); }
