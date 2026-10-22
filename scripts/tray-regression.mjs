// Isolated regression: real focus loss + native tray callbacks, deliberately delivered after blur.
// Explicit --no-startup + isolated data names the fixture separately from the live portable.
// No live-app shutdown, real credentials, Explorer injection or registry edits.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { focusTarget } from './focus-target.mjs';
const exe = path.resolve(process.argv[2] ?? 'target/release/opentoken.exe');
const dir = await mkdtemp(path.join(tmpdir(), 'opentoken-tray-race-'));
await writeFile(path.join(dir, 'settings.json'), JSON.stringify({ autoDiscover: false, codexHomes: [path.join(dir, 'empty')], claudeEnabled: false, dashboardPinned: false, launchAtStartup: false, apiPort: 16739 }));
const env = { ...process.env, OPENTOKEN_DATA_DIR: dir, OPENTOKEN_SOURCE_HOME:path.join(dir,'fixture-home'),APPDATA:path.join(dir,'fixture-roaming'),GH_CONFIG_DIR:path.join(dir,'fixture-gh'),GROK_HOME:path.join(dir,'fixture-grok'),KIMI_DATA_DIR:path.join(dir,'fixture-kimi'),MUSE_SESSIONS_DIR:path.join(dir,'fixture-muse'),XDG_DATA_HOME:path.join(dir,'fixture-xdg'), CODEX_HOME: path.join(dir, 'empty'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19223' };
const child = spawn(exe, ['show', '--no-startup'], { env, stdio: 'pipe' });
let errors = ''; child.stderr.on('data', b => errors += b.toString());
let browser, page, focus;
const sleep = ms => new Promise(r => setTimeout(r, ms));
async function wait(fn) { for (let i=0; i<500; i++) { if (child.exitCode !== null) throw Error(`Fixture exited: ${errors}`); const value = await fn().catch(() => false); if (value) return value; await sleep(10); } throw Error(`Timed out: ${errors}`); }
try {
  await wait(async () => (await fetch('http://127.0.0.1:16739/health')).ok);
  browser = await wait(async () => chromium.connectOverCDP('http://127.0.0.1:19223'));
  page = await wait(async () => browser.contexts().flatMap(c => c.pages()).find(p => /tauri\.localhost|tauri:\/\//.test(p.url())));
  const cfg = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_settings'));
  assert.equal(cfg.apiPort,16739); assert.equal(cfg.dashboardPinned,false); assert.deepEqual(cfg.codexHomes,[path.join(dir,'empty')]);
  const status = () => page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'));
  await wait(async () => (await status()).motion.phase === 'open');
  focus = await focusTarget();
  for (const press of ['trayDown', 'trayDouble']) {
    await focus.focus(); await focus.allow(child.pid);
    await wait(async () => (await status()).motion.phase === 'closing');
    // Snapshot must consider the in-flight closing panel open, even though its target is hidden.
    await focus[press](child.pid);
    await wait(async () => (await status()).motion.phase === 'hidden');
    const closed = await status();
    await focus.trayUp(child.pid); await sleep(450);
    const released = await status();
    assert.equal(released.motion.sequence, closed.motion.sequence, `${press}: release reopened/restarted an already auto-dismissed panel`);
    assert.equal(released.motion.phase,'hidden');
    // Open with a separate complete gesture, not a release-only fallback.
    await focus.allow(child.pid); await focus.trayDown(child.pid); await focus.trayUp(child.pid);
    await wait(async () => (await status()).motion.phase === 'open');
  }
  const beforeRepair = await status();
  await focus.trayDrop(child.pid);
  // No quota/manual refresh: the background health check must notice that the
  // OS registration was lost even though the app still owns its cached handle.
  const repaired = await (async () => { for (let i=0;i<120;i++) { const s=await status();if(s.trayRepairs>beforeRepair.trayRepairs && s.unconfirmedTrayIcons===0)return s;await sleep(250); } throw Error('Missing shell registration was not repaired'); })();
  assert.equal(repaired.nativeTrayIcons,beforeRepair.nativeTrayIcons,'repair must reuse the identity, not accumulate tray resources');
  await focus.allow(child.pid);await focus.trayDown(child.pid);await focus.trayUp(child.pid);
  await wait(async () => (await status()).motion.phase === 'hidden');
  await focus.allow(child.pid);await focus.trayDown(child.pid);await focus.trayUp(child.pid);
  await wait(async () => (await status()).motion.phase === 'open');
  await page.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
  await wait(async () => (await status()).dashboardPinned === true);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
  await page.getByRole('combobox', { name: 'Dashboard position', exact: true }).selectOption('bottom_left');
  await page.getByRole('button', { name: 'Save preferences', exact: true }).click();
  await page.getByRole('status').filter({ hasText: 'Preferences saved.' }).waitFor();
  const afterPreferences=await status();
  assert.equal(afterPreferences.dashboardPinned,true,'native preferences must retain the explicit pin');
  assert.equal(afterPreferences.alwaysOnTop,true);
  assert.equal(afterPreferences.position.x,afterPreferences.workArea.position.x+10);
  console.log('PASS: native pin and bottom-left placement persist through a real preference save after tray repair.');
  console.log('PASS: deleting only a synthetic child shell notification registration recovers without app restart, quota refresh, Explorer restart, duplicate resources or broken clicks.');
  console.log('PASS: normal/down and double-click second-press callbacks remain a single close after real unpinned blur wins, even when mouse-up arrives after the closing animation.');
} finally {
  await focus?.close().catch(() => {});
  if (page && child.exitCode === null) await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('quit')).catch(() => {});
  for (let i=0; child.exitCode === null && i<50; i++) await sleep(100);
  if (child.exitCode === null) { child.kill(); await new Promise(r => child.once('exit',r)); }
  await browser?.close().catch(() => {});
  await rm(dir,{recursive:true,force:true,maxRetries:10,retryDelay:100});
}
