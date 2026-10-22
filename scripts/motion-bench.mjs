// Interactive Windows desktop only. Isolated empty home; no real credentials/startup.
// Stop the real portable first. Optional executable argument enables a before/after comparison.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
const exe = path.resolve(process.argv[2] ?? 'target/release/opentoken.exe');
const dir = await mkdtemp(path.join(tmpdir(), 'opentoken-motion-'));
await writeFile(path.join(dir, 'settings.json'), JSON.stringify({ autoDiscover: false, codexEnabled: true, codexHomes: [path.join(dir, 'empty')], claudeEnabled: false, liveTracking: false, dashboardPinned: true, launchAtStartup: false, apiPort: 16739, theme: 'light' }));
const env = { ...process.env, OPENTOKEN_DATA_DIR: dir, CODEX_HOME: path.join(dir, 'empty'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19223' };
const child = spawn(exe, ['show', '--no-startup'], { env, stdio: 'pipe' });
let stderr = ''; child.stderr.on('data', b => stderr += b.toString());
let browser, page;
const sleep = ms => new Promise(r => setTimeout(r, ms));
async function wait(fn) { for (let i = 0; i < 100; i++) { if (child.exitCode !== null) throw Error(`Child exited: ${stderr}`); try { const result = await fn(); if (result) return result; } catch {} await sleep(100); } throw Error(`Timed out: ${stderr}`); }
const percentile = (values, p) => [...values].sort((a,b) => a-b)[Math.min(values.length-1, Math.floor(values.length*p))];
try {
  await wait(async () => (await fetch('http://127.0.0.1:16739/health')).ok);
  browser = await wait(async () => chromium.connectOverCDP('http://127.0.0.1:19223'));
  page = await wait(() => browser.contexts().flatMap(c => c.pages()).find(p => /tauri\.localhost|tauri:\/\//.test(p.url())));
  const settings = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_settings'));
  assert.equal(settings.apiPort, 16739); assert.equal(settings.codexEnabled, true); assert.equal(settings.claudeEnabled, false); assert.deepEqual(settings.codexHomes, [path.join(dir, 'empty')]);
  const status = () => page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'));
  const resting = await wait(async () => { const s = await status(); return s.motion.phase === 'open' ? s : false; });
  const results = [];
  const starts = [];
  // Warm native callbacks/assets, then measure without IPC polling during the animation.
  for (let i = 0; i < 5; i++) {
    for (const [command, phase] of [['hide_dashboard', 'hidden'], ['show_dashboard', 'open']]) {
      await page.evaluate(command => window.__TAURI_INTERNALS__.invoke(command), command);
      if (process.env.OPENTOKEN_TRANSITION_PROBE) {
        const first = await status();
        starts.push({ requested: command, phase: first.motion.phase, alpha: first.motion.alpha, nativeAlpha: first.material.opacity, layered: first.material.layered, slide: first.motion.slide, position: first.position, size: first.size });
        if (process.env.OPENTOKEN_TRANSITION_PROBE === 'assert' && ['opening','closing'].includes(first.motion.phase)) {
          assert.equal(first.material.layered, true, 'show must not clear the active opacity lease');
          assert.equal(first.material.opacity, first.motion.alpha, 'native and reported alpha must agree on the very first frame');
          assert.equal(first.position.y + first.size.height, resting.position.y + resting.size.height, 'dock edge must not drift');
        }
      }
      await sleep(550);
      const s = await status(); assert.equal(s.motion.phase, phase);
      assert.equal(s.material.layered, false); assert.equal(s.material.hostReveal, false);
      if (phase === 'open') { assert.deepEqual(s.position, resting.position); assert.deepEqual(s.size, resting.size); assert.deepEqual(s.viewport, resting.viewport); }
      if (i === 0) continue;
      const times = s.motion.frameTimesMs ?? [];
      assert.ok(times.every((v, index) => Number.isFinite(v) && (index === 0 || v >= times[index-1])));
      const gaps = times.slice(1).map((t, index) => t-times[index]);
      results.push({ phase, pacing: s.motion.pacing ?? 'legacy-sleep', frames: s.motion.frames, duration: s.motion.durationMs,
        elapsed: times.at(-1), medianGap: gaps.length ? percentile(gaps, .5) : undefined, maxGap: gaps.length ? Math.max(...gaps) : undefined });
    }
  }
  console.log(JSON.stringify({ executable: path.basename(exe), note: 'UI callback cadence, not a GPU presentation trace; isolated warm desktop, no in-motion IPC polling.', starts, results }, null, 2));
} finally {
  if (page && child.exitCode === null) await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('quit')).catch(() => {});
  for (let i = 0; child.exitCode === null && i < 50; i++) await sleep(100);
  if (child.exitCode === null) { child.kill(); await new Promise(r => child.once('exit', r)); }
  await browser?.close().catch(() => {});
  await rm(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}
