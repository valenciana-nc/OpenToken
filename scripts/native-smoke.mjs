// Runs the REAL release app with synthetic isolated sign-ins, never your credentials/startup entries.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, appendFile, readFile, rm, copyFile, rename } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { focusTarget } from './focus-target.mjs';
import { nativeLocalProviders } from './native-local-providers.mjs';
const dir = await mkdtemp(path.join(tmpdir(), 'opentoken-smoke-'));
const exe = path.resolve('target/release/opentoken.exe');
const homes = [path.join(dir, 'personal'), path.join(dir, 'work')];
const sessionReset = Math.floor(Date.now() / 1000) + 7200;
const weeklyReset = Math.floor(Date.now() / 1000) + 604800;
for (const [index, home] of homes.entries()) {
  await mkdir(home);
  const workspace = `fixture-${index}`; const email = `fixture-${index}@example.test`;
  const token = `e30.${Buffer.from(JSON.stringify({ email, 'https://api.openai.com/auth': { chatgpt_account_id: workspace } })).toString('base64url')}.sig`;
  await writeFile(path.join(home, 'auth.json'), JSON.stringify({ tokens: { account_id: workspace, access_token: token } }));
  await writeFile(path.join(home, 'account.json'), JSON.stringify({ workspace, email, percent: index ? 81 : 25, credits: 50, resets: 2, sessionReset, weeklyReset }));
}
const fixtureRate = { input: 2, cached: 0.1, output: 10, cacheWrite: 2.5, cacheWriteHour: 4 };
const sessionDir = path.join(homes[0], 'sessions'); await mkdir(sessionDir);
const sessionFile = path.join(sessionDir, 'fixture.jsonl');
const usageRecord = id => ({ type: 'token_usage_record', timestamp: new Date(Date.now() - 2000).toISOString(), payload: { session_id: 'fixture-session', response_id: id, usage: { input_tokens: 1500000, cached_input_tokens: 500000, output_tokens: 100000 } } });
await writeFile(sessionFile, [
  { type: 'session_meta', payload: { id: 'fixture-session' } },
  { type: 'turn_context', payload: { model: 'fixture-model' } }, usageRecord('one'),
].map(x => JSON.stringify(x)).join('\n') + '\n');
const bin = path.join(dir, 'bin'); await mkdir(bin);
await copyFile('target/debug/fake-codex.exe', path.join(bin, 'codex.exe'));
await writeFile(path.join(dir, 'settings.json'), JSON.stringify({ autoDiscover: false, codexHomes: homes, apiPort: 16737, theme: 'light', modelPrices: { 'fixture-model': fixtureRate } }));
const env = { ...process.env, OPENTOKEN_DATA_DIR: dir, OPENTOKEN_SOURCE_HOME:path.join(dir,'fixture-home'), GROK_HOME:path.join(dir,'fixture-grok'), KIMI_DATA_DIR:path.join(dir,'fixture-kimi'), MUSE_SESSIONS_DIR:path.join(dir,'fixture-muse'), XDG_DATA_HOME:path.join(dir,'fixture-xdg'), APPDATA: path.join(dir, 'isolated-roaming'), GH_CONFIG_DIR: path.join(dir,'isolated-gh'), CODEX_HOME: path.join(dir, 'empty-default'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19222' };
const pathKey = Object.keys(env).find(key => key.toLowerCase() === 'path') ?? 'PATH'; env[pathKey] = `${bin};${env[pathKey] ?? ''}`;
let child = spawn(exe, ['--background', '--no-startup'], { env, stdio: 'pipe' });
let stderr = ''; child.stderr.on('data', b => stderr += b.toString());
const sleep = ms => new Promise(r => setTimeout(r, ms));
async function wait(fn, attempts = 100) { let error; for (let i = 0; i < attempts; i++) { try { const v = await fn(); if (v) return v; } catch (e) { error = e; } await sleep(200); } if (browser) { const p = browser.contexts().flatMap(c => c.pages())[0]; try { console.error('Native diagnostic:', await p.evaluate(() => ({ scroll: document.querySelector('main')?.scrollTop, live: [...document.querySelectorAll('[role="status"]')].map(e => e.textContent), cards: [...document.querySelectorAll('.account-card')].map(e => ({ id: e.id, top: e.getBoundingClientRect().top, height: e.getBoundingClientRect().height, style: e.getAttribute('style'), pressed: e.querySelector('.account-drag-handle')?.getAttribute('aria-pressed') })) }))); } catch {} } throw error ?? new Error(`Timed out. ${stderr}`); }
async function providerSettings(page) {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Providers', exact: true }).click();
  await page.getByRole('heading', { name: 'Providers & settings', exact: true }).waitFor();
}
async function usagePage(page, provider) {
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption(provider);
}
async function accountSetup(page){const toggle=page.getByRole('button',{name:'Account setup (only if needed)',exact:true});if(await toggle.isVisible().catch(()=>false))await toggle.click();}
async function cli(verb) { const p = spawn(exe, [verb, '--no-startup'], { env, stdio: 'pipe' }); const code = await new Promise((resolve, reject) => { p.on('error', reject); p.on('exit', resolve); }); assert.equal(code, 0); }
let browser, focus;
async function activateMain() {
  await focus.focus(); await sleep(100);
  await focus.allow(child.pid); await focus.hide();
  await cli('show');
  await wait(async () => { const p = browser.contexts().flatMap(c => c.pages()).find(p => p.url().includes('tauri.localhost') || p.url().includes('tauri://')); if (!p) return false; const d = await p.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return d.visible && d.motion.phase === 'open'; });
  await focus.main(child.pid);
  await wait(async () => { const p = browser.contexts().flatMap(c => c.pages()).find(p => p.url().includes('tauri.localhost') || p.url().includes('tauri://')); if (!p) return false; const d = await p.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return d.visible && d.focused && d.motion.phase === 'open'; });
}
try {
  await wait(async () => { const r = await fetch('http://127.0.0.1:16737/health'); return r.ok; });
  const state = await wait(async () => { const s = await (await fetch('http://127.0.0.1:16737/v1/state')).json(); return s.accounts.length === 2 && s.accounts.every(a => a.status === 'ready') ? s : null; });
  assert.deepEqual(state.accounts.map(a => a.report.quotas[0].percent).sort((a,b) => a-b), [25, 81]);
  assert.equal((await fetch('http://127.0.0.1:16737/v1/state', { headers: { Origin: 'https://example.com' } })).status, 403);
  assert.equal((await fetch('http://127.0.0.1:16737/v1/state', { method: 'POST' })).status, 405);
  await cli('show');
  browser = await wait(async () => { try { return await chromium.connectOverCDP('http://127.0.0.1:19222'); } catch { return null; } });
  const page = await wait(async () => browser.contexts().flatMap(c => c.pages()).find(p => p.url().includes('tauri.localhost') || p.url().includes('tauri://')));
  await page.locator('.account-card').nth(1).waitFor();
  assert.equal(await page.locator('.spend-overview, .token-summary').count(), 0, 'quota cards must not contain the separate Usage page');
  const readsAtPin = await Promise.all(homes.map(home => readFile(path.join(home, 'provider-reads'), 'utf8')));
  const defaults = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'));
  assert.equal(defaults.alwaysOnTop, false);
  assert.equal(defaults.dashboardPinned, false);
  await page.getByRole('button', { name: 'Pin dashboard', exact: true }).waitFor();
  focus = await focusTarget();
  const desktopStatus = () => page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'));
  await activateMain();
  await focus.focus();
  await wait(async () => (await desktopStatus()).motion.phase === 'hidden');
  assert.equal((await fetch('http://127.0.0.1:16737/health')).status, 200, 'default unpinned dismissal must keep tracking alive');
  await activateMain();
  await page.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
  await wait(async () => { const s = await desktopStatus(); return s.alwaysOnTop === true && s.dashboardPinned === true; });
  assert.equal(JSON.parse(await readFile(path.join(dir, 'settings.json'), 'utf8')).dashboardPinned, true);
  const resting = await wait(async () => { const d = await desktopStatus(); return d.motion.phase === 'open' ? d : false; });
  assert.equal(resting.material.layered, false);
  assert.equal(resting.material.hostReveal, false);
  const stableLayout = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, scroll: document.querySelector('main').scrollTop, transform: getComputedStyle(document.querySelector('.surface')).transform }));
  async function assertWholeBubble(d) {
    if (!['opening', 'closing'].includes(d.motion.phase) || !d.motion.slide) return;
    assert.equal(d.material.hostReveal, true, 'the native glass popup itself must move, not only its contents');
    assert.equal(d.viewport.position.y, 0);
    assert.equal(d.material.layered, true);
    assert.equal(d.material.opacity, d.motion.alpha, 'native alpha must not flash/reset during motion');
    assert.equal(d.position.y + d.size.height, resting.position.y + resting.size.height, 'dock edge must stay fixed');
    assert.equal(d.size.height, resting.size.height - Math.abs(d.motion.slide), 'the real acrylic HWND must reveal from the dock edge');
    assert.ok(d.position.y - resting.position.y >= d.motion.slide, 'the actual window top must rise/fall with the full popup');
    assert.deepEqual(d.viewport.size, resting.viewport.size, 'panel motion must not resize/reflow its contents');
    assert.deepEqual(await page.evaluate(() => ({ width: innerWidth, height: innerHeight, scroll: document.querySelector('main').scrollTop, transform: getComputedStyle(document.querySelector('.surface')).transform })), stableLayout);
    assert.ok(d.position.y >= d.workArea.position.y);
    assert.ok(d.position.y + d.size.height <= d.workArea.position.y + d.workArea.size.height);
  }
  // Only synthetic UI over our own white fixture is captured; never the user's desktop/apps.
  await focus.backdrop(child.pid); await sleep(80);
  await focus.capture(child.pid, path.resolve('artifacts/native-flyout-open.png'));
  await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('hide_dashboard'));
  const closing = await desktopStatus();
  if (closing.motion.animationEnabled) {
    assert.equal(closing.motion.phase, 'closing');
    assert.equal(closing.material.layered, true);
    await sleep(100);
    const mid = await desktopStatus(); await assertWholeBubble(mid);
    console.log('Closing whole-popup frame:', { slide: mid.motion.slide, position: mid.position, size: mid.size, viewport: mid.viewport });
    try { await focus.capture(child.pid, path.resolve('artifacts/native-flyout-closing.png')); } catch { /* short transition may complete before capture on slow machines */ }
  }
  await wait(async () => (await desktopStatus()).motion.phase === 'hidden');
  await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('show_dashboard'));
  const opening = await desktopStatus();
  if (opening.motion.animationEnabled) {
    assert.equal(opening.motion.phase, 'opening');
    assert.ok(opening.motion.alpha < 255);
    assert.equal(opening.material.layered, true, 'Tao show must not drop WS_EX_LAYERED');
    assert.equal(opening.material.opacity, opening.motion.alpha);
    await sleep(120);
    const mid = await desktopStatus(); await assertWholeBubble(mid);
    assert.ok(Math.abs(opening.motion.slide) >= resting.viewport.size.height / 2, 'popup must begin at its dock edge, not just nudge a few pixels');
    console.log('Opening whole-popup frame:', { slide: mid.motion.slide, position: mid.position, size: mid.size, viewport: mid.viewport });
    try { await focus.capture(child.pid, path.resolve('artifacts/native-flyout-opening.png')); } catch { /* same frame-capture race as above */ }
  }
  await wait(async () => (await desktopStatus()).motion.phase === 'open');
  // Reverse both directions before completion. A stale close must never hide the reopened panel.
  for (let i = 0; i < 3; i++) {
    await page.evaluate(async () => {
      const invoke = window.__TAURI_INTERNALS__.invoke;
      await invoke('hide_dashboard'); await new Promise(r => setTimeout(r, 32));
      await invoke('show_dashboard'); await new Promise(r => setTimeout(r, 32));
      await invoke('hide_dashboard'); await new Promise(r => setTimeout(r, 32));
      await invoke('show_dashboard');
    });
    await wait(async () => (await desktopStatus()).motion.phase === 'open');
    const d = await desktopStatus();
    assert.equal(d.visible, true); assert.equal(d.motion.alpha, 255); assert.equal(d.material.layered, false);
    assert.equal(d.motion.slide, 0); assert.equal(d.material.hostReveal, false); assert.deepEqual(d.viewport, resting.viewport);
    assert.deepEqual(d.position, resting.position, 'motion drifted away from the exact saved dock');
    assert.equal(d.material.systemBackdrop, resting.material.systemBackdrop);
    assert.equal(d.material.cornerPreference, resting.material.cornerPreference);
    assert.ok(d.position.y + d.size.height <= d.workArea.position.y + d.workArea.size.height, 'animation must never paint over the taskbar');
  }
  // Exercise real Tao extended-style rebuilds DURING a full entry, not just at its endpoint.
  await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('hide_dashboard'));
  await wait(async () => (await desktopStatus()).motion.phase === 'hidden');
  await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('show_dashboard'));
  for (const pinned of [false, true]) {
    await page.evaluate(pinned => window.__TAURI_INTERNALS__.invoke('set_dashboard_pinned', { pinned }), pinned);
    const active = await desktopStatus();
    if (active.motion.phase === 'opening') await assertWholeBubble(active);
  }
  await wait(async () => (await desktopStatus()).motion.phase === 'open');
  const idle = await desktopStatus();
  await sleep(150);
  const stillIdle = await desktopStatus();
  assert.equal(stillIdle.motion.frames, idle.motion.frames, 'animation worker must stop at idle');
  assert.deepEqual(stillIdle.motion.frameTimesMs, idle.motion.frameTimesMs);
  assert.ok(stillIdle.motion.frameTimesMs.length <= 256);
  assert.ok(stillIdle.motion.frameTimesMs.every((t, i, ts) => Number.isFinite(t) && (!i || t >= ts[i-1])));
  console.log('Native flyout motion:', { enabled: opening.motion.animationEnabled, opening: opening.motion.durationMs, closing: closing.motion.durationMs, pacing: stillIdle.motion.pacing, restoredLayered: stillIdle.material.layered });
  await focus.focus();
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).focused === false);
  await sleep(400);
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible, true, 'explicitly pinned panel must survive a real click into another process');
  await activateMain();
  await page.getByRole('button', { name: 'Unpin dashboard', exact: true }).click();
  await wait(async () => { const d = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return d.alwaysOnTop === false && d.dashboardPinned === false; });
  assert.equal(JSON.parse(await readFile(path.join(dir, 'settings.json'), 'utf8')).dashboardPinned, false);
  await focus.focus();
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible === false);
  assert.equal((await fetch('http://127.0.0.1:16737/health')).status, 200, 'focus-loss dismissal must not stop background tracking');
  const dismissed = await desktopStatus();
  await focus.trayUp(child.pid); await sleep(280);
  assert.equal((await desktopStatus()).motion.sequence, dismissed.motion.sequence, 'stray release must not reopen an auto-dismissed popup');
  // Replay native notifications for EACH registered fixture badge, with real focus loss between down/up.
  for (const index of [0, 1]) {
    await focus.allow(child.pid); await focus.trayDown(child.pid, index); await focus.trayUp(child.pid, index);
    await wait(async () => (await desktopStatus()).motion.phase === 'open');
    const opened = await desktopStatus();
    await focus.trayDown(child.pid, index); await focus.focus(); await sleep(350);
    const held = await desktopStatus();
    assert.equal(held.trayGesture, true); assert.equal(held.motion.phase, 'open', 'held tray press must defer blur dismissal');
    assert.equal(held.motion.sequence, opened.motion.sequence);
    await focus.trayUp(child.pid, index);
    await wait(async () => (await desktopStatus()).motion.phase === 'hidden');
    await sleep(280);
    const closed = await desktopStatus();
    assert.equal(closed.motion.sequence, opened.motion.sequence + 1, 'one tray click must cause exactly one close');
    await focus.trayUp(child.pid, index); await sleep(100);
    assert.equal((await desktopStatus()).motion.sequence, closed.motion.sequence, 'duplicate release must not toggle twice');
  }
  await focus.trayDown(child.pid, 0); await focus.trayUp(child.pid, 1); await sleep(100);
  assert.equal((await desktopStatus()).motion.phase, 'hidden', 'dragging between badges must not activate either badge');
  assert.equal((await desktopStatus()).trayGesture, false);
  await activateMain();
  await focus.trayDown(child.pid, 0); await focus.focus(); await focus.trayUp(child.pid, 1);
  await wait(async () => (await desktopStatus()).motion.phase === 'hidden');
  assert.equal((await desktopStatus()).trayGesture, false, 'cancelled gesture must not leave an unpinned panel stuck open');
  console.log('Native tray gestures: both badges, blur during held press, one close per click, duplicate/stray/cross-badge releases rejected; cancelled gesture still dismisses on blur.');
  await activateMain();
  const settingsFile = path.join(dir, 'settings.json'), heldSettings = path.join(dir, 'settings.pin-test.json');
  await rename(settingsFile, heldSettings); await mkdir(settingsFile);
  try {
    await page.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
    await page.getByRole('alert').filter({ hasText: 'Dashboard pin could not be saved' }).waitFor();
    await wait(async () => { const d = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return d.alwaysOnTop === false && d.dashboardPinned === false; });
    assert.equal(await page.getByRole('button', { name: 'Pin dashboard', exact: true }).getAttribute('aria-pressed'), 'false');
  } finally { await rm(settingsFile, { recursive: true }); await rename(heldSettings, settingsFile); }
  await page.getByRole('button', { name: 'Dismiss error', exact: true }).click();
  await providerSettings(page);
  assert.equal(await page.getByRole('button', { name: 'Browse folders', exact: true }).count(), 0, 'legacy manual-folder setup must not return');
  assert.equal(await page.getByLabel('Login folder', { exact: true }).count(), 0);
  await page.getByRole('button', { name: 'Use existing Codex', exact: true }).waitFor();
  assert.equal(await page.getByRole('button',{name:'Sign in with Codex',exact:true}).count(),0,'source discovery must not default to another login');
  // Retain native owned-dialog regression coverage through the compatibility command,
  // not a folder-management form in the production UI.
  await page.evaluate(() => { window.__pickerTest = window.__TAURI_INTERNALS__.invoke('choose_login_folder'); });
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).modalInteractions === 1);
  await sleep(350);
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible, true, 'an owned folder dialog must not dismiss the unpinned parent');
  await focus.trayDown(child.pid); await focus.trayUp(child.pid); await sleep(100);
  assert.equal((await desktopStatus()).modalInteractions, 1);
  assert.equal((await desktopStatus()).visible, true, 'tray click must not hide the dialog parent or steal its focus');
  await focus.escape(child.pid);
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).modalInteractions === 0);
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible, true);
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  assert.deepEqual(await Promise.all(homes.map(home => readFile(path.join(home, 'provider-reads'), 'utf8'))), readsAtPin, 'pin changes must not trigger extra quota reads');
  await page.reload();
  await page.getByRole('button', { name: 'Pin dashboard', exact: true }).waitFor();
  // Restore native AND WebView focus after the picker/reload before testing real keyboard input.
  await page.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).alwaysOnTop === true);
  await activateMain();
  assert.equal(await page.evaluate(() => document.hasFocus()), true);
  await usagePage(page, 'codex');
  assert.equal(await page.locator('.provider-grid, .account-card, [role="progressbar"]').count(), 0);
  await wait(async () => (await page.locator('.donut-total strong').textContent()) === '$3.05');
  assert.equal(await page.locator('.donut-total small').textContent(), 'API-equivalent · estimate', 'token value must not be presented as subscription charges');
  assert.equal(await page.locator('.spend-legend dd').first().textContent(), '$3.05');
  assert.equal(await page.locator('.period-control button').count(), 3, 'full spending section was removed again');
  assert.match(await page.locator('.token-summary').textContent(), /Today.*1.6M tokens.*Last 30 days.*1.6M tokens/);
  const spendSurfaces = await page.locator('.spend-surface').evaluateAll(es => es.map(e => { const s = getComputedStyle(e); return { background: s.backgroundColor, border: s.borderTopWidth, shadow: s.boxShadow }; }));
  for (const surface of spendSurfaces) assert.deepEqual(surface, { background: 'rgba(0, 0, 0, 0)', border: '0px', shadow: 'none' });
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  assert.equal(await page.getByRole('tab').count(), 12, 'curated provider grid is incomplete');
  assert.equal(await page.getByRole('tab', { name: 'Codex', exact: true }).evaluate(e => getComputedStyle(e).backgroundColor), 'rgb(222, 222, 222)', 'selected provider must use a neutral gray square');
  assert.equal(await page.getByRole('tab', { name: 'Codex', exact: true }).evaluate(e => getComputedStyle(e).color), 'rgb(27, 27, 27)', 'selected provider needs readable theme-appropriate text');
  assert.ok((await page.locator('.track > div').evaluateAll(es => es.map(e => getComputedStyle(e).backgroundColor))).every(color => color === 'rgb(0, 120, 212)'), 'changing the selection tile must not recolor quota bars');
  assert.deepEqual(await page.getByRole('tab').allTextContents(), ['Codex', 'Claude', 'Antigravity', 'Grok', 'Muse Code', 'Cursor', 'Copilot', 'Devin', 'Z.ai', 'DeepSeek', 'Kimi', 'OpenCode'], 'keep existing providers and append Z.ai, DeepSeek, Kimi and OpenCode');
  assert.equal(await page.getByRole('tab', { name: 'Overview', exact: true }).count(), 0);
  const opencode = await page.getByRole('tab', { name: 'OpenCode', exact: true }).boundingBox();
  const add = await page.getByRole('button', { name: 'Add provider', exact: true }).boundingBox();
  assert.ok(Math.abs(opencode.y - add.y) < 1 && add.x > opencode.x, 'Add must sit next to OpenCode');
  const addMark = page.locator('.add-provider-mark');
  assert.equal(await addMark.getAttribute('aria-hidden'), 'true');
  assert.deepEqual(await addMark.evaluate(e => { const s = getComputedStyle(e); return { width: s.width, height: s.height, border: s.borderTopWidth, radius: s.borderRadius, background: s.backgroundColor, shadow: s.boxShadow, color: s.color }; }), {
    width: '31px', height: '31px', border: '0px', radius: '0px', background: 'rgba(0, 0, 0, 0)', shadow: 'none', color: await page.getByRole('tab', { name: 'OpenCode', exact: true }).evaluate(e => getComputedStyle(e).color),
  }, 'Add must use a bare monochrome glyph matching the other marks, not a boxed icon');
  for (const name of ['Amp', 'ElevenLabs', 'Zed', 'Perplexity', 'Pi', 'TypeSafe']) assert.equal(await page.getByRole('tab', { name, exact: true }).count(), 0);
  assert.equal(await page.locator('.token-summary').count(), 0);
  const surfaces = await page.locator('.metrics-surface, .spend-surface').evaluateAll(es => es.map(e => { const s = getComputedStyle(e); return { background: s.backgroundColor, border: s.borderTopWidth, shadow: s.boxShadow }; }));
  for (const surface of surfaces) assert.deepEqual(surface, { background: 'rgba(0, 0, 0, 0)', border: '0px', shadow: 'none' }, 'usage and spending must be integrated, not separate boxes');
  await page.getByRole('tab', { name: 'Cursor', exact: true }).click();
  await page.getByRole('button', { name: 'Use existing Cursor', exact: true }).waitFor();
  assert.equal(await page.locator('.cursor-sign-in input, .cursor-sign-in select, .cursor-sign-in ol').count(), 0);
  assert.equal(await page.locator('.cursor-sign-in .primary').count(), 1);
  assert.equal(await page.getByRole('button', { name: 'Use existing Cursor', exact: true }).isEnabled(), true);
  assert.ok((await page.locator('.cursor-sign-in').boundingBox()).height < 190, 'default sign-in must be compact, not a setup wall');
  assert.equal(await page.locator('.sign-in-caption, .sign-in-info').count(), 0);
  assert.match(await page.getByRole('button', { name: 'Use existing Cursor', exact: true }).getAttribute('title'), /Cursor desktop sign-in read-only.*unofficial dashboard/);
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_cursor_login_status'))).phase, 'idle');
  const rejectedStart = await page.evaluate(async () => { try { await window.__TAURI_INTERNALS__.invoke('start_cursor_sign_in', { consent: false }); return ''; } catch (error) { return String(error); } });
  assert.match(rejectedStart, /start it explicitly/);
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('cancel_cursor_sign_in'))).phase, 'idle');
  const consentError = await page.evaluate(async () => { try { await window.__TAURI_INTERNALS__.invoke('connect_cursor_personal', { consent: false }); return ''; } catch (error) { return String(error); } });
  assert.match(consentError, /Allow read-only access/);
  // Child APPDATA points only at our empty synthetic profile, never the real Cursor sign-in.
  const missingCursor = await page.evaluate(async () => { try { await window.__TAURI_INTERNALS__.invoke('connect_cursor_personal', { consent: true }); return ''; } catch (error) { return String(error); } });
  assert.match(missingCursor, /No Cursor desktop sign-in was found/);
  assert.deepEqual((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_settings'))).connections, []);
  await page.screenshot({ path: 'artifacts/native-cursor-personal-connect.png', fullPage: true });
  await page.getByRole('button', { name: 'Enterprise setup', exact: true }).click();
  assert.match(await page.getByRole('tabpanel').textContent(), /Enterprise admin API.*personal Cursor plans are not supported/s);
  assert.equal(await page.getByLabel('Enterprise admin API key', { exact: true }).getAttribute('type'), 'password');
  await page.getByRole('button', { name: 'Verify & connect', exact: true }).scrollIntoViewIfNeeded();
  assert.equal(await page.getByRole('button', { name: 'Verify & connect', exact: true }).isVisible(), true, 'connection form must remain usable in the fixed-height popup');
  await page.screenshot({ path: 'artifacts/native-connection-cursor.png', fullPage: true });
  const invalidConnection = await page.evaluate(async () => { try { await window.__TAURI_INTERNALS__.invoke('connect_provider', { input: { provider: 'cursor', token: 'short', memberEmail: 'member@example.test' } }); return ''; } catch (error) { return String(error); } });
  assert.match(invalidConnection, /provider-issued API key or token/);
  assert.deepEqual((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_settings'))).connections, []);
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_state'))).accounts.length, 2, 'invalid API connection must not add borrowed accounts');
  const rejectedRemoval = await page.evaluate(async () => { try { await window.__TAURI_INTERNALS__.invoke('disconnect_provider', { id: 'not-an-api-connection' }); return ''; } catch (error) { return String(error); } });
  assert.match(rejectedRemoval, /no longer exists/);
  const rejectedUrl = await page.evaluate(async () => { try { await window.__TAURI_INTERNALS__.invoke('open_provider_page', { provider: 'https://malicious.invalid/', docs: false }); return ''; } catch (error) { return String(error); } });
  assert.match(rejectedUrl, /No supported provider page/);
  await page.getByRole('tab', { name: 'Copilot', exact: true }).click();
  await page.getByRole('button', { name: 'Use existing Copilot', exact: true }).waitFor();
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_github_login_status'))).phase,'idle');
  assert.equal(await page.evaluate(async()=>{try{await window.__TAURI_INTERNALS__.invoke('start_github_sign_in',{consent:false});return false;}catch{return true;}}),true);
  await page.getByRole('button', { name: 'Advanced personal billing', exact: true }).click();
  await page.getByLabel('Fine-grained personal access token', { exact: true }).waitFor();
  assert.match(await page.getByRole('tabpanel').textContent(), /Plan: read/);
  assert.match(await page.getByRole('tabpanel').textContent(), /Organization-paid usage/);
  assert.equal(await page.getByLabel('Fine-grained personal access token', { exact: true }).getAttribute('type'), 'password');
  await page.getByRole('button', { name: 'Manage Copilot accounts', exact: true }).click();
  assert.equal(await page.getByRole('combobox', { name: 'Provider', exact: true }).inputValue(), 'copilot');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  assert.equal(await page.locator('.donut, .token-summary').count(), 0);
  await page.getByRole('tab',{name:'Grok',exact:true}).click();
  await page.getByRole('button',{name:'Use existing Grok',exact:true}).waitFor();
  assert.equal((await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_grok_login_status'))).phase,'idle');
  assert.equal(await page.evaluate(async()=>{try{await window.__TAURI_INTERNALS__.invoke('start_grok_sign_in',{consent:false});return false;}catch{return true;}}),true);
  assert.equal(await page.locator('input[type="password"]').count(),0);
  assert.equal(await page.locator('.token-summary').count(), 0);
  await usagePage(page, 'grok');
  assert.match(await page.locator('.token-stat strong').first().textContent(), /^— tokens$/);
  assert.equal(await page.getByRole('progressbar').count(), 0);
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await page.getByRole('tab',{name:'Muse Code',exact:true}).click();
  await page.getByRole('button',{name:'Track local usage',exact:true}).waitFor();
  assert.equal(await page.getByRole('progressbar').count(),0);
  for (const name of ['Antigravity', 'Devin']) {
    await page.getByRole('tab', { name, exact: true }).click();
    assert.equal(await page.getByRole('progressbar').count(), 0, 'disconnected providers must not borrow Codex quotas');
    assert.equal(await page.locator('.donut, .token-summary, input[type="password"]').count(), 0);
    assert.equal(await page.getByRole('tab', { name, exact: true }).locator('.provider-usage-stripe').count(), 0);
    await page.getByRole('button',{name:`Use existing ${name}`,exact:true}).waitFor();
  }
  await page.getByRole('tab', { name: 'Codex', exact: true }).click();
  await usagePage(page, 'codex');
  await wait(async () => (await page.locator('.donut-total strong').textContent()) === '$3.05');
  await page.getByRole('button', { name: 'Today', exact: true }).click();
  assert.match(await page.locator('.spend-unavailable').textContent(), /Estimated token value/);
  await page.getByRole('button', { name: 'Yesterday', exact: true }).click();
  await wait(async () => (await page.locator('.donut-total strong').textContent()) === '—');
  assert.match(await page.locator('.spend-unavailable').textContent(), /yesterday/);
  await page.getByRole('button', { name: 'Today', exact: true }).click();
  assert.equal(await page.locator('.statusbar, .status-accounts').count(), 0, 'simulated desktop bar must not be inside the app');
  assert.equal(await page.locator('.app').evaluate(e => getComputedStyle(e).padding), '0px', 'screenshot side gutters were not removed');
  assert.equal(await page.locator('html').evaluate(e => getComputedStyle(e).backgroundColor), 'rgba(0, 0, 0, 0)', 'WebView must let the native acrylic show through');
  assert.equal(await page.locator('.forecast-tick').count(), 0, 'native app must not invent quota forecasts');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await page.locator('.account-card').nth(1).waitFor();
  assert.equal(await page.locator('.account-card').count(), 2);
  const logos = await page.locator('.account-heading .provider-logo').evaluateAll(elements => elements.map(e => { const style = getComputedStyle(e); return { background: style.backgroundColor, border: style.borderTopWidth, shadow: style.boxShadow }; }));
  assert.equal(logos.length, 2);
  for (const logo of logos) assert.deepEqual(logo, { background: 'rgba(0, 0, 0, 0)', border: '0px', shadow: 'none' }, 'provider logo must not have a square tile');
  const left = await page.getByRole('progressbar', { name: 'Session remaining', exact: true }).evaluateAll(elements => elements.map(e => Number(e.getAttribute('aria-valuenow'))).sort((a,b) => a-b));
  assert.deepEqual(left, [19, 75], 'remaining UI does not match the underlying used quota');
  const card = page.locator('.account-card').first();
  const credits = card.getByText('Credits', { exact: true });
  const resets = card.getByText('Rate limit resets', { exact: true });
  assert.equal(await credits.isVisible(), false, 'credits must be collapsed by default');
  assert.equal(await resets.isVisible(), false, 'reset allowances must be collapsed by default');
  assert.equal(await card.locator('.quota-list .value-row').count(), 0);
  await card.getByRole('button', { name: 'Expand Codex details', exact: true }).click();
  assert.equal(await credits.isVisible(), true);
  assert.equal(await resets.isVisible(), true);
  assert.match(await card.locator('.account-details').textContent(), /50 credits · \$2\.00/);
  assert.match(await card.locator('.account-details').textContent(), /2 available/);
  await card.getByRole('button', { name: 'Collapse Codex details', exact: true }).click();
  assert.equal(await credits.isVisible(), false);
  assert.equal(await resets.isVisible(), false);
  const originalOrder = await page.locator('.account-card').evaluateAll(cards => cards.map(c => c.id));
  const firstHandle = page.locator('.account-drag-handle').first();
  await firstHandle.click({ trial: true });
  await firstHandle.focus(); await page.keyboard.press('Space');
  await wait(async () => (await firstHandle.getAttribute('aria-pressed')) === 'true');
  await wait(async () => (await page.locator('[role="status"][aria-live="assertive"]').textContent()).includes('position 1 of 2'));
  // KeyboardSensor installs its key listener in a zero-delay macrotask after pickup.
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 0)));
  await page.keyboard.press('ArrowDown');
  await wait(async () => (await page.locator('[role="status"][aria-live="assertive"]').textContent()).includes('position 2 of 2'));
  await page.keyboard.press('Escape');
  await wait(async () => (await page.locator('[role="status"][aria-live="assertive"]').textContent()).includes('Move cancelled'));
  assert.deepEqual(await page.locator('.account-card').evaluateAll(cards => cards.map(c => c.id)), originalOrder);
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible, true, 'Escape during reordering should cancel the move, not hide the window');
  // Finish the visible cancellation animation before beginning the next keyboard gesture.
  await wait(async () => page.locator('.account-card').evaluateAll(cards => cards.every(card => getComputedStyle(card).transform === 'none')));
  await firstHandle.click({ trial: true });
  await firstHandle.focus(); await page.keyboard.press('Space');
  await wait(async () => (await firstHandle.getAttribute('aria-pressed')) === 'true');
  await wait(async () => (await page.locator('[role="status"][aria-live="assertive"]').textContent()).includes('position 1 of 2'));
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 0)));
  await page.keyboard.press('ArrowDown');
  await wait(async () => (await page.locator('[role="status"][aria-live="assertive"]').textContent()).includes('position 2 of 2'));
  await page.keyboard.press('Space');
  await wait(async () => (await page.locator('.account-card').first().getAttribute('id')) === originalOrder[1]);
  await wait(async () => JSON.parse(await readFile(path.join(dir, 'settings.json'), 'utf8')).accountOrder?.[0] === originalOrder[1]);
  await page.reload(); await page.locator('.account-card').nth(1).waitFor();
  assert.deepEqual(await page.locator('.account-card').evaluateAll(cards => cards.map(c => c.id)), [...originalOrder].reverse(), 'card order was not restored');
  await usagePage(page, 'codex');
  await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('hide_dashboard'));
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible === false);
  const pushedAt = Date.now();
  await appendFile(sessionFile, JSON.stringify(usageRecord('two')) + '\n');
  await wait(async () => (await page.locator('.donut-total strong').textContent()) === '$6.10');
  assert.ok(Date.now() - pushedAt < 3500, 'hidden-window token totals should be pushed without IPC polling, reload or manual refresh');
  await cli('show');
  const changed = JSON.parse(await readFile(path.join(homes[0], 'account.json'), 'utf8')); changed.credits = 43;
  await writeFile(path.join(homes[0], 'account.json'), JSON.stringify(changed));
  await cli('refresh');
  await wait(async () => { const s = await (await fetch('http://127.0.0.1:16737/v1/state')).json(); return s.accounts.some(a => a.report?.quotas.some(q => q.id === 'credits' && q.value === 43)); });
  const spending = await wait(async () => { const data = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_spending')); return data.periods['30 Days'].creditChanges[0]?.decrease === 7 ? data : null; });
  assert.equal(spending.periods['30 Days'].tokens, 3200000);
  assert.ok(Math.abs(spending.periods['30 Days'].estimatedUsd - 6.1) < 1e-8);
  assert.equal(spending.periods['30 Days'].creditChanges[0].decrease, 7);
  await appendFile(sessionFile, [
    { type: 'turn_context', payload: { model: 'unpriced-fixture' } },
    { type: 'token_usage_record', timestamp: new Date(Date.now() - 1000).toISOString(), payload: { response_id: 'unpriced-call', usage: { input_tokens: 1500, cached_input_tokens: 500, output_tokens: 100 } } },
  ].map(x => JSON.stringify(x)).join('\n') + '\n');
  await wait(async () => (await page.locator('.donut-total strong').textContent()) === '≥$6.10');
  assert.match(await page.locator('.spend-unavailable').textContent(), /Lower bound:.*unpriced-fixture/);
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  const liveAccount = state.accounts.find(a => a.account.identity.workspace === 'fixture-0').account.id;
  const rateRecord = (percent, proof) => ({ type: 'event_msg', timestamp: new Date().toISOString(), payload: { type: 'token_count', info: null, ...(proof ? { account: { workspace: 'fixture-0', email: 'fixture-0@example.test', user: null } } : {}), rate_limits: { limit_id: 'codex', primary: { used_percent: percent, window_minutes: 300, resets_at: sessionReset } } } });
  const readsBefore = (await readFile(path.join(homes[0], 'provider-reads'), 'utf8')).trim().split('\n').length;
  const quotaAt = Date.now();
  await appendFile(sessionFile, JSON.stringify(rateRecord(90, true)) + '\n');
  await wait(async () => { const data = await (await fetch('http://127.0.0.1:16737/v1/state')).json(); const a = data.accounts.find(a => a.account.id === liveAccount); return a.report.source === 'cli_log' && a.report.quotas[0].percent === 90; });
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).trayRemaining[liveAccount] === 10);
  assert.ok(Date.now() - quotaAt < 3500, 'identity-proven log quota should update the native mini badge promptly');
  await wait(async () => (await page.getByRole('tab', { name: 'Codex', exact: true }).locator('.provider-usage-stripe i').evaluate(e => e.style.width)) === '10%');
  assert.equal((await readFile(path.join(homes[0], 'provider-reads'), 'utf8')).trim().split('\n').length, readsBefore, 'a proven local quota hint should not make an immediate extra provider call');
  const authoritative = JSON.parse(await readFile(path.join(homes[0], 'account.json'), 'utf8')); authoritative.percent = 93;
  await writeFile(path.join(homes[0], 'account.json'), JSON.stringify(authoritative));
  await appendFile(sessionFile, JSON.stringify(rateRecord(99, false)) + '\n');
  await sleep(1200);
  const provisional = await (await fetch('http://127.0.0.1:16737/v1/state')).json();
  assert.notEqual(provisional.accounts.find(a => a.account.id === liveAccount).report.quotas[0].percent, 99, 'anonymous logs must not assign percentages to the current account');
  // Wait for the real production 60-second budget, not an accelerated test-only configuration.
  await wait(async () => { const data = await (await fetch('http://127.0.0.1:16737/v1/state')).json(); const a = data.accounts.find(a => a.account.id === liveAccount); return a.report.source === 'provider' && a.report.quotas[0].percent === 93; }, 400);
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).trayRemaining[liveAccount] === 7);
  await wait(async () => (await page.getByRole('tab', { name: 'Codex', exact: true }).locator('.provider-usage-stripe i').evaluate(e => e.style.width)) === '7%');
  assert.equal((await readFile(path.join(homes[0], 'provider-reads'), 'utf8')).trim().split('\n').length, readsBefore + 1, 'activity confirmation should be a single budgeted provider read');
  const tracking = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_tracking'));
  assert.equal(tracking.mode, 'watching'); assert.ok(tracking.lastActivityAt.codex);
  const desktop = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'));
  assert.equal(desktop.accountTrayIcons, 2);
  assert.equal(desktop.providerTrayIcons, 0);
  assert.equal(desktop.nativeTrayIcons, 2, 'the default Fluent style should have one percentage badge per identity');
  if (desktop.material.systemBackdrop !== null) assert.equal(desktop.material.systemBackdrop, 3, 'Windows DWM acrylic is not active');
  if (desktop.material.cornerPreference !== null) assert.equal(desktop.material.cornerPreference, 2, 'native rounded corners are not enabled');
  assert.equal(desktop.placeholderRegistered, false, 'placeholder leaked into Tauri registry after real account icons appeared');
  const margin = Math.round(10 * await page.evaluate(() => devicePixelRatio));
  assert.equal(desktop.position.x + desktop.size.width, desktop.workArea.position.x + desktop.workArea.size.width - margin, 'dashboard should default to bottom-right');
  assert.equal(desktop.position.y + desktop.size.height, desktop.workArea.position.y + desktop.workArea.size.height - margin, 'dashboard overlaps the taskbar');
  await mkdir('artifacts', { recursive: true });
  await page.screenshot({ path: 'artifacts/native-overview-light.png', fullPage: true });
  await usagePage(page, 'codex');
  await page.locator('.spend-overview').scrollIntoViewIfNeeded();
  await page.screenshot({ path: 'artifacts/native-spending-light.png', fullPage: true });
  // CDP can operate a hidden WebView. Re-establish the native presented state
  // before testing immediate docking; preference saves intentionally do NOT
  // reopen a popup dismissed while background quota checks were running.
  await activateMain();
  // Earlier fixture-only IPC pin cycles and background checks need not leave
  // the later preference/docking test's explicit-pinned precondition intact.
  if (!(await desktopStatus()).dashboardPinned) await page.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
  await wait(async () => { const d=await desktopStatus();return d.dashboardPinned && d.alwaysOnTop; });
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
  await page.getByRole('combobox', { name: 'Tray icon style', exact: true }).selectOption('provider_pair');
  await page.getByRole('spinbutton', { name: 'Loopback port', exact: true }).fill('16738');
  await page.getByRole('combobox', { name: 'Dashboard position', exact: true }).selectOption('bottom_left');
  await page.getByRole('button', { name: 'Save preferences' }).click();
  await page.getByRole('status').filter({ hasText: 'Preferences saved' }).waitFor();
  const saved = JSON.parse(await readFile(path.join(dir, 'settings.json'), 'utf8'));
  assert.equal(saved.trayStyle, 'provider_pair');
  assert.equal(saved.dashboardPinned, true, 'the preference save must retain an explicitly set native/UI pin');
  await wait(async () => { const s = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return s.accountTrayIcons === 2 && s.providerTrayIcons === 2 && s.nativeTrayIcons === 4 && !s.placeholderRegistered; });
  assert.equal(saved.theme, 'dark'); assert.equal(saved.apiPort, 16738); assert.equal(saved.launchAtStartup, false); assert.equal(saved.flyoutPosition, 'bottom_left');
  const right = await wait(async () => { const d = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return d.visible && d.motion.phase === 'open' && d.position.x === d.workArea.position.x + margin ? d : null; });
  assert.equal(right.position.x, right.workArea.position.x + margin, 'saved bottom-left placement was not applied');
  await wait(async () => (await fetch('http://127.0.0.1:16738/health')).ok);
  await mkdir('artifacts', { recursive: true });
  await page.screenshot({ path: 'artifacts/native-settings-dark.png', fullPage: true });
  await page.getByRole('combobox', { name: 'Tray icon style', exact: true }).selectOption('badge');
  await page.getByRole('button', { name: 'Save preferences' }).click();
  await wait(async () => { const s = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return s.accountTrayIcons === 2 && s.providerTrayIcons === 0 && s.nativeTrayIcons === 2 && !s.placeholderRegistered; });
  await cli('refresh');
  await page.evaluate(async () => { const s = await window.__TAURI_INTERNALS__.invoke('get_settings'); await window.__TAURI_INTERNALS__.invoke('save_settings', { settings: { ...s, codexHomes: [], flyoutPosition: 'bottom_left' } }); });
  await wait(async () => { const s = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return s.accountTrayIcons === 0 && s.providerTrayIcons === 0 && s.nativeTrayIcons === 1 && s.placeholderRegistered; });
  await page.evaluate(async homes => { const s = await window.__TAURI_INTERNALS__.invoke('get_settings'); await window.__TAURI_INTERNALS__.invoke('save_settings', { settings: { ...s, codexHomes: homes } }); }, homes);
  await wait(async () => { const s = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return s.accountTrayIcons === 2 && s.providerTrayIcons === 0 && s.nativeTrayIcons === 2 && !s.placeholderRegistered; });
  await activateMain();
  await page.getByRole('button', { name: 'Unpin dashboard', exact: true }).click();
  await wait(async () => { const d = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return d.alwaysOnTop === false && d.dashboardPinned === false; });
  await page.getByRole('button', { name: 'Collapse dashboard', exact: true }).click();
  await wait(async () => (await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible === false);
  assert.equal((await fetch('http://127.0.0.1:16738/health')).status, 200, 'hiding the flyout stopped the backend');
  await cli('show');
  await cli('quit');
  await wait(() => child.exitCode !== null);
  assert.equal(child.exitCode, 0);
  await browser.close().catch(() => {}); browser = undefined;
  child = spawn(exe, ['--background', '--no-startup'], { env, stdio: 'pipe' });
  child.stderr.on('data', b => stderr += b.toString());
  await wait(async () => (await fetch('http://127.0.0.1:16738/health')).ok);
  browser = await wait(async () => { try { return await chromium.connectOverCDP('http://127.0.0.1:19222'); } catch { return null; } });
  await activateMain();
  const restored = await wait(async () => browser.contexts().flatMap(c => c.pages()).find(p => p.url().includes('tauri.localhost') || p.url().includes('tauri://')));
  await restored.getByRole('button', { name: 'Pin dashboard', exact: true }).waitFor();
  const pinAfterRestart = await restored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'));
  assert.equal(pinAfterRestart.dashboardPinned, false); assert.equal(pinAfterRestart.alwaysOnTop, false, 'saved pin preference must override the window config after a full process restart');
  await focus.focus();
  await wait(async () => (await restored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible === false);
  await activateMain();
  await restored.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
  await wait(async () => (await restored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).dashboardPinned === true);
  await cli('quit'); await wait(() => child.exitCode !== null); assert.equal(child.exitCode, 0);
  await browser.close().catch(() => {}); browser = undefined;
  child = spawn(exe, ['--background', '--no-startup'], { env, stdio: 'pipe' });
  child.stderr.on('data', b => stderr += b.toString());
  await wait(async () => (await fetch('http://127.0.0.1:16738/health')).ok);
  browser = await wait(async () => { try { return await chromium.connectOverCDP('http://127.0.0.1:19222'); } catch { return null; } });
  await activateMain();
  const pinnedRestored = await wait(async () => browser.contexts().flatMap(c => c.pages()).find(p => p.url().includes('tauri.localhost') || p.url().includes('tauri://')));
  const explicitPin = await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'));
  assert.equal(explicitPin.dashboardPinned, true); assert.equal(explicitPin.alwaysOnTop, true, 'explicit saved pin must override the new unpinned config on restart');
  const repairBefore = explicitPin.trayRepairs;
  await focus.trayDrop(child.pid, 0);
  await wait(async () => { const d=await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status')); return d.trayRepairs>repairBefore && d.unconfirmedTrayIcons===0 && d.nativeTrayIcons===2; }, 180);
  assert.equal((await fetch('http://127.0.0.1:16738/health')).status, 200, 'tray repair must not restart or interrupt the engine');
  await focus.focus(); await sleep(450);
  assert.equal((await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible, true);
  await activateMain();
  await providerSettings(pinnedRestored);
  const beforeCli = await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_settings'));
  await pinnedRestored.getByRole('button',{name:'Use existing Codex',exact:true}).click();
  await pinnedRestored.getByRole('status').filter({hasText:'2 existing Codex accounts verified'}).waitFor();
  assert.deepEqual((await pinnedRestored.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_settings'))).codexHomes,beforeCli.codexHomes);
  await assert.rejects(readFile(path.join(dir,'sign-ins/codex/pending.json')),'existing-session reuse must not allocate a managed sign-in');
  assert.equal((await pinnedRestored.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_cli_login_status',{provider:'codex'}))).phase,'idle');
  const sourceCatalogue=await pinnedRestored.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_sources'));assert.equal(sourceCatalogue.sources.length,12);assert.equal(JSON.stringify(sourceCatalogue).includes('accessToken'),false);
  await sleep(15100); // Respect the shared provider verification gate before testing explicit recovery.
  for (const provider of ['codex', 'claude']) {
    const idle = await pinnedRestored.evaluate(provider => window.__TAURI_INTERNALS__.invoke('get_cli_login_status', { provider }), provider);
    assert.equal(idle.phase, 'idle');
    const denied = await pinnedRestored.evaluate(async provider => { try { await window.__TAURI_INTERNALS__.invoke('start_cli_sign_in', { provider, consent: false }); return false; } catch { return true; } }, provider);
    assert.equal(denied, true);
  }
  await pinnedRestored.getByRole('combobox', { name: 'Provider', exact: true }).selectOption('claude');
  await pinnedRestored.getByRole('button', { name: 'Use existing Claude', exact: true }).waitFor();
  assert.equal(await pinnedRestored.getByLabel('Login folder', { exact: true }).count(), 0);
  await pinnedRestored.getByRole('combobox', { name: 'Provider', exact: true }).selectOption('codex');
  await accountSetup(pinnedRestored);
  await pinnedRestored.getByRole('button', { name: 'Sign in with Codex', exact: true }).click();
  await wait(async () => (await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_cli_login_status', { provider: 'codex' }))).phase === 'waiting');
  const pending = JSON.parse(await readFile(path.join(dir, 'sign-ins/codex/pending.json'), 'utf8'));
  const signInHome = path.join(dir, 'sign-ins/codex', pending.id);
  await wait(async () => (await readFile(path.join(signInHome, 'login-started'), 'utf8')) === 'synthetic-only');
  assert.equal(await readFile(path.join(signInHome, 'login-env-safe'), 'utf8'), 'true');
  assert.deepEqual(JSON.parse(await readFile(path.join(signInHome, 'login-console.json'),'utf8')), {stdin:true,stdout:true,stderr:true}, 'a sign-in launched by the GUI must get actual console handles, not inherited pipes/null handles');
  const duplicate = await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('start_cli_sign_in', { provider: 'codex', consent: true }));
  assert.equal(duplicate.phase, 'waiting');
  await pinnedRestored.getByRole('button', { name: 'Cancel sign-in', exact: true }).click();
  await writeFile(path.join(signInHome, 'complete-sign-in'), 'synthetic-ready'); await sleep(1400);
  assert.equal((await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_cli_login_status', { provider: 'codex' }))).phase, 'cancelled');
  assert.deepEqual((await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_settings'))).codexHomes, beforeCli.codexHomes);
  await pinnedRestored.getByRole('button', { name: 'Sign in with Codex', exact: true }).click();
  await pinnedRestored.getByRole('button', { name: 'Settings', exact: true }).click();
  await pinnedRestored.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('light');
  await wait(async () => (await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_cli_login_status', { provider: 'codex' }))).phase === 'connected');
  assert.equal(await pinnedRestored.getByRole('combobox', { name: 'Theme', exact: true }).inputValue(), 'light', 'sign-in completion must preserve unsaved drafts');
  const afterCli = await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_settings'));
  assert.equal(afterCli.codexHomes.length, beforeCli.codexHomes.length + 1);
  assert.equal(afterCli.theme, beforeCli.theme); assert.equal(afterCli.launchAtStartup, false);
  const nativeCli = await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_state'));
  assert.equal(nativeCli.accounts.filter(a => a.account.email === 'new-signin@example.test' && a.status === 'ready').length, 1);
  await pinnedRestored.getByRole('button', { name: 'Providers', exact: true }).click();
  await pinnedRestored.getByRole('heading', { name: 'Providers & settings', exact: true }).waitFor();
  await accountSetup(pinnedRestored);
  await pinnedRestored.getByRole('button', { name: 'Sign in with Codex', exact: true }).scrollIntoViewIfNeeded();
  // Capture only the synthetic release WebView here, never unrelated desktop windows.
  await pinnedRestored.screenshot({ path: 'artifacts/native-simple-accounts.png' });
  await pinnedRestored.getByRole('button', { name: 'Disconnect new-signin@example.test', exact: true }).click();
  await wait(async () => (await pinnedRestored.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_state'))).accounts.length === 2);
  assert.equal((await readFile(path.join(signInHome, 'account.json'), 'utf8')).includes('new-signin@example.test'), true, 'disconnect must leave provider-owned sign-in data untouched');
  const museFile=path.join(env.MUSE_SESSIONS_DIR,'2026/10/18/synthetic-session/session.jsonl');await mkdir(path.dirname(museFile),{recursive:true});
  const museRecord=id=>JSON.stringify({schema_version:1,record_type:'event',payload_type:'runtime.session',payload_schema_version:1,id,recorded_at:Date.now()*1000,payload:{event:{kind:'model_completed',model:'synthetic-unpriced-muse',usage:{input_tokens:100,output_tokens:25,cached_tokens:40,reasoning_tokens:10}}}})+'\n';
  await writeFile(museFile,museRecord('muse-native-one'));
  await pinnedRestored.evaluate(()=>window.__TAURI_INTERNALS__.invoke('set_history_source',{provider:'muse',enabled:true,consent:true}));
  const museTokens=async()=>{const t=await pinnedRestored.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_tracking'));return t.data?.periods.Today.providers.find(p=>p.provider==='muse')?.tokens;};
  await wait(async()=>await museTokens()===125);
  await pinnedRestored.getByRole('button',{name:'Collapse dashboard',exact:true}).click();
  await wait(async()=>!(await pinnedRestored.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).visible);
  await appendFile(museFile,museRecord('muse-native-two'));await wait(async()=>await museTokens()===250);
  assert.equal((await pinnedRestored.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_state'))).accounts.some(a=>a.account.provider==='muse'),false);
  await pinnedRestored.evaluate(()=>window.__TAURI_INTERNALS__.invoke('set_history_source',{provider:'muse',enabled:false,consent:true}));await sleep(1600);await appendFile(museFile,museRecord('muse-native-three'));await sleep(2200);assert.equal(await museTokens(),250,'stopping local monitoring retains imported totals but must stop new ingestion');
  await nativeLocalProviders({page:pinnedRestored,dir,env,wait,focus,pid:child.pid,processId:()=>child.pid,restart:async()=>{
    await cli('quit');await wait(()=>child.exitCode!==null);assert.equal(child.exitCode,0);await browser.close();
    child=spawn(exe,['--background','--no-startup'],{env,stdio:'pipe'});child.stderr.on('data',b=>stderr+=b.toString());
    const restoredPort=JSON.parse(await readFile(path.join(dir,'settings.json'),'utf8')).apiPort;
    await wait(async()=>{try{return(await fetch(`http://127.0.0.1:${restoredPort}/health`)).ok;}catch{return false;}});
    browser=await wait(async()=>{try{return await chromium.connectOverCDP('http://127.0.0.1:19222');}catch{return false;}});
    return await wait(async()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')||p.url().includes('tauri://')));
  }});
  await cli('quit'); await wait(() => child.exitCode !== null); assert.equal(child.exitCode, 0);
  console.log('PASS: discovery-first Codex reuse without managed-profile allocation or login; explicit recovery remains separate; Muse records/hidden pushes/stop/retention without account auth or fake quota.');
  console.log('PASS: single-action native Codex sign-in with synthetic official-CLI transport, duplicate start, cancellation, retry, background auto-verification, settings/draft preservation and monitoring-only disconnect; Claude button/explicit-consent boundary; no manual folder UI.');
  console.log('PASS: native acrylic, rounded corners, transparent WebView without screenshot bars, bare provider logos without square tiles, default unpinned panel dismisses on a real external-process click, explicit pin stays open; unpinned focus loss dismisses without stopping the API, owned folder dialog stays open/cancels safely, saved auto-dismiss behavior restored after restart, header pin beside refresh with native topmost state, targeted persistence and failure rollback without quota reads, full-process unpinned restore, down-chevron collapse without quitting, Fluent percentage badges, live badge/pair style switching and resource cleanup, local token/cost indexing, incremental history, credit-change tracking, persistent keyboard card reordering and Escape cancellation, twelve matching provider marks including Z.ai/DeepSeek/Kimi/OpenCode with a matching bare monochrome Add icon beside OpenCode, the unchanged priority row and no Overview, unified Providers & settings with preserved drafts, separate Usage page with provider-scoped Today/Last 30 days tokens and the full unboxed spend section, background pushed token updates while hidden, identity-proven live quota and native mini badge/stripe changes, anonymous-log rejection and real 60-second activity-triggered provider confirmation without refresh clicks, compact single-button Cursor sign-in, cached native flow state and explicit-start/missing-session rejection with isolated APPDATA, advanced Cursor Enterprise / personal Copilot connection forms, credential-free unsupported-provider options, invalid native connect/disconnect rejection without state changes, fixed browser-destination allowlist, unsupported-provider honesty, credits/resets hidden until usage details expand, bottom-right/left work-area docking, WebView/IPC, settings, API rebind, browser rejection, single instance, hide/reopen, exit.');
} finally {
  if (focus) await focus.close().catch(() => {});
  if (browser) await browser.close().catch(() => {});
  if (child.exitCode === null) child.kill();
  await sleep(500); await rm(dir, { recursive: true, force: true });
}
