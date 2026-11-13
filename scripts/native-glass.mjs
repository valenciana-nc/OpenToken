// Real DWM/WebView acceptance against ONLY our synthetic native backdrop. No production credentials/CDP.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, rm, copyFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { focusTarget } from './focus-target.mjs';

const dir = await mkdtemp(path.join(tmpdir(), 'opentoken-glass-'));
const output = path.join(process.env.LOCALAPPDATA, 'OpenToken-build', 'glass-20261025');
await mkdir(output, { recursive: true });
const home = path.join(dir, 'synthetic-codex'), bin = path.join(dir, 'bin');
await mkdir(home); await mkdir(bin);
const workspace = 'glass-fixture', email = 'glass-fixture@example.test';
const token = `e30.${Buffer.from(JSON.stringify({ email, 'https://api.openai.com/auth': { chatgpt_account_id: workspace } })).toString('base64url')}.sig`;
await writeFile(path.join(home, 'auth.json'), JSON.stringify({ tokens: { account_id: workspace, access_token: token } }));
await writeFile(path.join(home, 'account.json'), JSON.stringify({ workspace, email, percent: 25, credits: 50, resets: 2, sessionReset: Math.floor(Date.now() / 1000) + 7200, weeklyReset: Math.floor(Date.now() / 1000) + 604800 }));
await copyFile('target/debug/fake-codex.exe', path.join(bin, 'codex.exe'));
await writeFile(path.join(dir, 'settings.json'), JSON.stringify({ autoDiscover: false, codexHomes: [home], codexEnabled: true, claudeEnabled: false, apiPort: 16751, theme: 'light', launchAtStartup: false }));
const env = { ...process.env, OPENTOKEN_DATA_DIR: dir, OPENTOKEN_SOURCE_HOME: path.join(dir, 'empty-home'), APPDATA: path.join(dir, 'empty-roaming'), GH_CONFIG_DIR: path.join(dir, 'empty-gh'), GROK_HOME: path.join(dir, 'empty-grok'), KIMI_DATA_DIR: path.join(dir, 'empty-kimi'), MUSE_SESSIONS_DIR: path.join(dir, 'empty-muse'), XDG_DATA_HOME: path.join(dir, 'empty-xdg'), CODEX_HOME: path.join(dir, 'empty-codex'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19231' };
const pathKey = Object.keys(env).find(key => key.toLowerCase() === 'path') ?? 'PATH';
env[pathKey] = `${bin};${env[pathKey] ?? ''}`;
await mkdir(env.OPENTOKEN_SOURCE_HOME);
const child = spawn(path.resolve('target/release/opentoken.exe'), ['--no-startup'], { env, stdio: 'pipe' });
let errors = ''; child.stderr.on('data', b => errors += b);
let browser, page, focus;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function wait(fn) {
  for (let i = 0; i < 300; i++) {
    if (child.exitCode !== null) throw new Error(`Synthetic child exited: ${errors}`);
    try { const value = await fn(); if (value) return value; } catch { /* Startup/transition. */ }
    await sleep(200);
  }
  throw new Error(`Native glass wait timed out: ${errors}`);
}
try {
  await wait(async () => (await fetch('http://127.0.0.1:16751/health')).ok);
  browser = await wait(() => chromium.connectOverCDP('http://127.0.0.1:19231'));
  page = await wait(() => browser.contexts().flatMap(context => context.pages()).find(page => /tauri\.localhost|tauri:\/\//.test(page.url())));
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  const status = () => invoke('get_desktop_status');
  await page.locator('.surface').waitFor();
  await page.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
  await wait(async () => { const current = await status(); return current.dashboardPinned && current.alwaysOnTop; });
  focus = await focusTarget();
  await wait(async () => (await status()).motion.phase === 'open');
  await focus.focus(); await focus.allow(child.pid);
  await invoke('show_dashboard');
  await wait(async () => (await status()).motion.phase === 'open');
  await focus.backdrop(child.pid);
  await focus.main(child.pid);
  await wait(async () => (await status()).focused === true);

  async function transparentRegions() {
    const tint = await page.locator('.surface').evaluate(element => getComputedStyle(element).backgroundColor);
    const theme = await page.locator('html').getAttribute('data-theme');
    assert.equal(tint, theme === 'light' ? 'rgba(243, 243, 243, 0.8)' : 'rgba(32, 32, 32, 0.84)', 'one taskbar-weight tint must cover the whole native surface');
    const regions = await page.locator('html, body, #root, .app, .panel-header, .provider-grid, main, footer, .save-bar').evaluateAll(elements => elements.map(element => ({
      region: element.className || element.tagName,
      background: getComputedStyle(element).backgroundColor,
      opacity: getComputedStyle(element).opacity,
      blur: getComputedStyle(element).backdropFilter,
    })));
    for (const region of regions) {
      assert.equal(region.background, 'rgba(0, 0, 0, 0)', JSON.stringify(region));
      assert.equal(region.opacity, '1', JSON.stringify(region));
      assert.equal(region.blur, 'none', 'desktop blur must come from DWM, not CSS');
    }
  }
  async function scrollbarTrack(theme) {
    const main = page.locator('main');
    const thumb = theme === 'light' ? 'rgba(0, 0, 0, 0.36)' : 'rgba(255, 255, 255, 0.38)';
    assert.equal(await main.evaluate(element => getComputedStyle(element).scrollbarColor), `${thumb} rgba(0, 0, 0, 0)`);
    const probes = await main.evaluate(element => {
      const rect = element.getBoundingClientRect(), height = element.clientHeight;
      const thumbTop = height * element.scrollTop / element.scrollHeight;
      const thumbHeight = Math.max(20, height * height / element.scrollHeight);
      const bottomGap = height - thumbTop - thumbHeight;
      const y = rect.top + (thumbTop > bottomGap ? thumbTop / 2 : thumbTop + thumbHeight + bottomGap / 2);
      const width = Math.max(6, element.offsetWidth - element.clientWidth);
      return { overflow: element.scrollHeight > height, x: rect.right - width / 2, reference: rect.right - width - 10, y, width: innerWidth, height: innerHeight };
    });
    assert.equal(probes.overflow, true, 'exercise a real overflowing settings page');
    await focus.color(0x226cce); await focus.main(child.pid); await focus.park(); await sleep(500);
    const file = path.join(output, `native-scrollbar-${theme}.png`);
    await focus.capture(child.pid, file);
    const base64 = (await readFile(file)).toString('base64');
    const native = await status();
    const compared = await page.evaluate(async ({ base64, probes, viewportWidth }) => {
      const image = new Image(); image.src = `data:image/png;base64,${base64}`; await image.decode();
      const canvas = document.createElement('canvas'); canvas.width = image.width; canvas.height = image.height;
      const ctx = canvas.getContext('2d'); ctx.drawImage(image, 0, 0);
      const scale = viewportWidth / probes.width;
      const xOffset = (image.width - probes.width * scale) / 2, yOffset = (image.height - probes.height * scale) / 2;
      const sample = x => {
        const pixels = ctx.getImageData(Math.round(xOffset + x * scale), Math.round(yOffset + probes.y * scale) - 2, 1, 5).data;
        return [0, 1, 2].map(channel => { let total = 0; for (let i = channel; i < pixels.length; i += 4) total += pixels[i]; return total / 5; });
      };
      return { track: sample(probes.x), adjacent: sample(probes.reference) };
    }, { base64, probes, viewportWidth: native.viewport.size.width });
    const difference = Math.max(...compared.track.map((channel, i) => Math.abs(channel - compared.adjacent[i])));
    assert.ok(difference <= 4, `scrollbar track painted a separate plate: ${JSON.stringify(compared)}`);
    console.log(`PASS ${theme}: real overflowing scrollbar track matches the adjacent native glass (maximum RGB difference ${difference}).`);
  }
  async function samples(file) {
    const base64 = (await readFile(file)).toString('base64');
    return page.evaluate(async base64 => {
      const image = new Image(); image.src = `data:image/png;base64,${base64}`; await image.decode();
      const canvas = document.createElement('canvas'); canvas.width = image.width; canvas.height = image.height;
      const ctx = canvas.getContext('2d'); ctx.drawImage(image, 0, 0);
      // Reject a capture of only the opaque backdrop (e.g. wrong native z-order).
      const full = ctx.getImageData(0, 0, image.width, image.height).data;
      let low = 255, high = 0;
      for (let i = 0; i < full.length; i += 4) { low = Math.min(low, full[i]); high = Math.max(high, full[i]); }
      if (high - low < 60) throw new Error('Native capture did not contain the visible popup UI');
      // Blank left gutter beside the header, main body and footer, inside native frame margins.
      return [22, Math.round(image.height / 2), image.height - 22].map(y => {
        const pixels = ctx.getImageData(17, y - 2, 4, 5).data;
        return [0, 1, 2].map(channel => { let sum = 0; for (let i = channel; i < pixels.length; i += 4) sum += pixels[i]; return sum / (pixels.length / 4); });
      });
    }, base64);
  }
  for (const theme of ['light', 'dark']) {
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption(theme);
    await page.getByRole('button', { name: 'Save preferences', exact: true }).click();
    await page.getByRole('status').filter({ hasText: 'Preferences saved.' }).waitFor();
    await wait(async () => await page.locator('html').getAttribute('data-theme') === theme && (await status()).motion.phase === 'open');
    await transparentRegions();
    await scrollbarTrack(theme);
    await page.getByRole('button', { name: 'Providers', exact: true }).click(); await transparentRegions();
    await page.getByRole('button', { name: 'Back to providers', exact: true }).click(); await transparentRegions();
    await page.getByRole('button', { name: 'Usage', exact: true }).click(); await transparentRegions();
    await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
    await wait(async () => await page.locator('.provider-usage-stripe > i').count() > 0);
    const meterColors = await page.locator('.provider-usage-stripe > i').evaluateAll(elements => elements.map(element => getComputedStyle(element).backgroundColor));
    assert.deepEqual(meterColors, [theme === 'light' ? 'rgb(0, 120, 212)' : 'rgb(96, 205, 255)'], 'brand mini meters must use the app blue');
    const native = await status();
    assert.equal(native.dashboardPinned, true);
    assert.equal(native.alwaysOnTop, true, 'synthetic panel must remain above its opaque safety backdrop');
    assert.equal(native.material.systemBackdrop, 3, 'Windows 11 must use native transient-window acrylic');
    assert.equal(native.material.layered, false, 'settled text/controls must not be faded with whole-window opacity');
    assert.equal(native.material.hostReveal, false);
    assert.equal(native.focused, true);
    const captures = [];
    for (const [name, color] of [['blue', 0x226cce], ['warm', 0xd96d38]]) {
      await focus.color(color);
      // Native theme/save/show operations can alter z-order. Reassert that our panel,
      // not merely its still-live WebView, is above the opaque safety backdrop.
      await focus.main(child.pid); await focus.park(); await sleep(1200);
      const file = path.join(output, `native-glass-${theme}-${name}.png`);
      await focus.capture(child.pid, file); captures.push(await samples(file));
    }
    const differences = captures[0].map((sample, i) => Math.max(...sample.map((channel, c) => Math.abs(channel - captures[1][i][c]))));
    for (const [i, difference] of differences.entries()) {
      assert.ok(difference < 40 && (theme === 'dark' || difference > 6), `native ${theme} ${['header', 'body', 'footer'][i]} must not return to clear glass: ${JSON.stringify({ captures, differences })}`);
    }
    // Require actual transmission in the user's light-theme reference. A native dark
    // material can resolve solid on Windows; report that observation, not a fake blur pass.
    if (theme === 'dark' && differences.every(difference => difference === 0)) {
      assert.ok(captures.flat(2).every(channel => channel <= 64), 'solid dark result must remain a neutral dark surface');
      console.log('OBSERVED: dark native samples were solid; no dark backdrop transmission is claimed for this run.');
    }
    console.log(`PASS ${theme}: blue mini meter, DWM acrylic=3, uniform taskbar-weight tint, opaque text; measured header/body/footer backdrop differences: ${JSON.stringify(differences)}`);
  }
  assert.equal((await fetch('http://127.0.0.1:16751/health')).status, 200);
  console.log(`Native-only synthetic captures: ${output}`);
} catch (error) {
  if (page) {
    console.error('Native glass diagnostic:', await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')).catch(() => null));
    await page.screenshot({ path: path.join(output, 'webview-diagnostic.png') }).catch(() => {});
  }
  throw error;
} finally {
  await focus?.close().catch(() => {});
  if (page && child.exitCode === null) await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('quit')).catch(() => {});
  for (let i = 0; child.exitCode === null && i < 100; i++) await sleep(100);
  if (child.exitCode === null) { child.kill(); await new Promise(resolve => child.once('exit', resolve)); }
  await browser?.close().catch(() => {});
  await rm(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 500 });
}
