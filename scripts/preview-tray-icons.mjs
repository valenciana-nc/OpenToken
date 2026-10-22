// First export real Rust-rendered examples by setting OPENTOKEN_ICON_PREVIEW_DIR
// to target/badge-previews and running cargo test -p opentoken fluent_badges.
import { chromium } from '@playwright/test';
import { readFile, mkdir } from 'node:fs/promises';
const browser = await chromium.launch({ channel: 'msedge' });
try {
  const page = await browser.newPage({ viewport: { width: 720, height: 330 }, deviceScaleFactor: 2 });
  const sections = await Promise.all(['light', 'dark'].map(async theme => {
    const images = await Promise.all([0, 8, 29, 72, 100].map(async n => {
      const png = await readFile(`target/badge-previews/badge-${theme}-${n}.png`);
      const uri = `data:image/png;base64,${png.toString('base64')}`;
      return `<div class="sample"><img width="48" height="48" src="${uri}"><div class="sizes">${[16,20,24,32].map(size => `<img width="${size}" height="${size}" src="${uri}">`).join('')}</div></div>`;
    }));
    return `<section class="${theme}"><h2>${theme === 'light' ? 'Light taskbar' : 'Dark taskbar'}</h2><div class="grid">${images.join('')}</div></section>`;
  }));
  await page.setContent(`<style>*{box-sizing:border-box}body{margin:0;padding:16px;background:#e8eef4;color:#26384b;font:12px 'Segoe UI',sans-serif}header{display:flex;justify-content:space-between;align-items:center;margin:0 0 12px}h1{font-size:16px;font-weight:600;margin:0}small{color:#657489}main{display:grid;grid-template-columns:1fr 1fr;gap:14px}section{border:1px solid #23354c14;border-radius:12px;padding:16px 12px 20px;background:#f4f8fc}section.dark{background:#202b39;color:#e4ecf7}h2{font-size:12px;margin:0 0 20px;font-weight:500}.grid{display:flex;gap:12px;justify-content:space-between}.sample{display:flex;align-items:center;flex-direction:column;gap:16px}.sizes{display:flex;align-items:center;flex-direction:column;gap:10px}img{display:block}footer{margin-top:10px;color:#657489;font-size:10px}</style><header><h1>Fluent percentage badges</h1><small>Design examples · not live usage</small></header><main>${sections.join('')}</main><footer>Large preview, then native-size samples at 16 / 20 / 24 / 32 pixels. Generated from the real Rust renderer.</footer>`);
  await page.evaluate(() => Promise.all([...document.images].map(image => image.decode())));
  await mkdir('artifacts', { recursive: true });
  await page.screenshot({ path: 'artifacts/tray-badge-preview.png', fullPage: true });
} finally { await browser.close(); }
