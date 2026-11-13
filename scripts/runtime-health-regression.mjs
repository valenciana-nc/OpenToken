// Suspend ONLY the window thread of the synthetic child spawned here. Never touches the live app/task.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';

const dir = await mkdtemp(path.join(tmpdir(), 'opentoken-health-'));
const exe = path.resolve('target/release/opentoken.exe');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const env = { ...process.env, OPENTOKEN_DATA_DIR: dir, OPENTOKEN_SOURCE_HOME: path.join(dir, 'empty-home'), APPDATA: path.join(dir, 'empty-roaming'), GH_CONFIG_DIR: path.join(dir, 'empty-gh'), GROK_HOME: path.join(dir, 'empty-grok'), KIMI_DATA_DIR: path.join(dir, 'empty-kimi'), MUSE_SESSIONS_DIR: path.join(dir, 'empty-muse'), XDG_DATA_HOME: path.join(dir, 'empty-xdg'), CODEX_HOME: path.join(dir, 'empty-codex'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19229' };
await mkdir(env.OPENTOKEN_SOURCE_HOME);
await writeFile(path.join(dir, 'settings.json'), JSON.stringify({ autoDiscover: false, codexEnabled: false, claudeEnabled: false, apiPort: 16749, launchAtStartup: false }));
const child = spawn(exe, ['--background', '--no-startup'], { env, stdio: 'pipe' });
let stderr = ''; child.stderr.on('data', b => stderr += b);
const exited = new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal })));
let browser;
async function wait(fn, timeout = 120000) {
  const until = Date.now() + timeout;
  while (Date.now() < until) {
    if (child.exitCode !== null) throw new Error(`Child exited early: ${stderr}`);
    try { const result = await fn(); if (result) return result; } catch { /* startup */ }
    await sleep(200);
  }
  throw new Error(`Timed out: ${stderr}`);
}
function powershell(script) {
  return new Promise((resolve, reject) => {
    const proc = spawn('powershell.exe', ['-NoProfile', '-Command', script], { stdio: 'pipe' });
    let output = '', error = '';
    proc.stdout.on('data', b => output += b); proc.stderr.on('data', b => error += b);
    proc.on('error', reject); proc.on('exit', code => code === 0 ? resolve(output.trim()) : reject(new Error(error)));
  });
}
try {
  await wait(async () => (await fetch('http://127.0.0.1:16749/health')).ok);
  browser = await wait(() => chromium.connectOverCDP('http://127.0.0.1:19229'));
  const page = await wait(() => browser.contexts().flatMap(context => context.pages()).find(page => page.url().includes('tauri')));
  // Concurrent state/status requests and refresh bursts must not deadlock tray reconciliation.
  await page.evaluate(async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    for (let i = 0; i < 50; i++) {
      await Promise.all([invoke('get_desktop_status'), invoke('get_state'), invoke('refresh')]);
    }
  });
  const status = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'));
  assert.equal(status.nativeTrayIcons, 1);
  assert.equal(status.unconfirmedTrayIcons, 0);
  // Span multiple heartbeat/probe periods before deliberately blocking the UI.
  await sleep(16000);
  assert.equal((await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_desktop_status'))).nativeTrayIcons, 1);
  console.log('Healthy isolated UI survived concurrent status/refresh bursts and heartbeat intervals.');
  await browser.close(); browser = undefined;
  await powershell(`$ErrorActionPreference='Stop'; Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OwnedHang {
  public delegate bool Callback(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr p);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenThread(uint access, bool inherit, uint id);
  [DllImport("kernel32.dll")] static extern uint SuspendThread(IntPtr h);
  [DllImport("kernel32.dll")] static extern uint ResumeThread(IntPtr h);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  public static uint Suspend(uint pid) {
    uint thread=0;
    EnumWindows((h,p) => { uint owner; uint id=GetWindowThreadProcessId(h,out owner); if(owner==pid){thread=id;return false;} return true; }, IntPtr.Zero);
    if(thread==0) throw new Exception("Owned child has no window thread");
    IntPtr handle=OpenThread(2,false,thread);
    if(handle==IntPtr.Zero) throw new Exception("Cannot open owned child thread");
    try { uint count=SuspendThread(handle); if(count==uint.MaxValue) throw new Exception("Suspend failed"); if(count!=0){ResumeThread(handle);throw new Exception("Thread was already suspended");} }
    finally { CloseHandle(handle); }
    return thread;
  }
}
'@; [OwnedHang]::Suspend(${child.pid})`);
  const started = Date.now();
  await sleep(10000);
  assert.equal((await fetch('http://127.0.0.1:16749/health')).status, 200, 'API liveness alone must not count as UI responsiveness');
  const result = await Promise.race([exited, sleep(140000).then(() => { throw new Error('UI watchdog did not terminate the stalled child'); })]);
  assert.equal(result.code, 1);
  const failure = await readFile(path.join(dir, 'logs', 'failures.log'), 'utf8');
  assert.match(failure, /UI heartbeat delayed for 30 seconds/);
  assert.match(failure, /UI heartbeat unresponsive for 120 seconds/);
  assert.match(failure, /stage=/);
  console.log(`Deliberately stalled UI exited with code 1 after ${Math.round((Date.now() - started) / 1000)}s; warning and fatal diagnostics persisted.`);
} finally {
  if (browser) await browser.close().catch(() => {});
  if (child.exitCode === null) { child.kill(); await exited; }
  await rm(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 500 });
}
