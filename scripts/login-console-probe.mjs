// Synthetic, isolated proof of console-handle ownership; never real authentication.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp,mkdir,copyFile,writeFile,readFile,rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
const exe=path.resolve(process.argv[2]??'target/release/opentoken.exe');
const dir=await mkdtemp(path.join(tmpdir(),'opentoken-console-'));const bin=path.join(dir,'bin');await mkdir(bin);
await copyFile('target/debug/fake-codex.exe',path.join(bin,'codex.exe'));
await writeFile(path.join(dir,'settings.json'),JSON.stringify({codexEnabled:false,claudeEnabled:false,autoDiscover:false,apiPort:16745}));
const env={...process.env,OPENTOKEN_DATA_DIR:dir,OPENTOKEN_SOURCE_HOME:path.join(dir,'fixture-home'),GH_CONFIG_DIR:path.join(dir,'fixture-gh'),GROK_HOME:path.join(dir,'fixture-grok'),KIMI_DATA_DIR:path.join(dir,'fixture-kimi'),MUSE_SESSIONS_DIR:path.join(dir,'fixture-muse'),XDG_DATA_HOME:path.join(dir,'fixture-xdg'),APPDATA:path.join(dir,'roaming'),CODEX_HOME:path.join(dir,'default'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=19229'};
const key=Object.keys(env).find(k=>k.toLowerCase()==='path')??'PATH';env[key]=`${bin};${env[key]}`;
const child=spawn(exe,['--background','--no-startup'],{env,stdio:'pipe'});let browser,page;
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(fn){for(let i=0;i<150;i++){try{const result=await fn();if(result)return result;}catch{}await sleep(100);}throw Error('Synthetic console probe timed out');}
try{
 await wait(async()=> (await fetch('http://127.0.0.1:16745/health')).ok);
 browser=await wait(async()=>chromium.connectOverCDP('http://127.0.0.1:19229'));
 page=await wait(async()=>browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri.localhost|tauri:\/\//.test(p.url())));
 await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('start_cli_sign_in',{provider:'codex',consent:true}));
 const pending=await wait(async()=>JSON.parse(await readFile(path.join(dir,'sign-ins/codex/pending.json'),'utf8')));
 const result=await wait(async()=>JSON.parse(await readFile(path.join(dir,'sign-ins/codex',pending.id,'login-console.json'),'utf8')));
 console.log(JSON.stringify({executable:exe,console:result}));
 if(process.argv.includes('--assert')&&!Object.values(result).every(Boolean))throw Error('Login did not receive usable console handles');
}finally{
 if(page)await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('quit')).catch(()=>{});
 for(let i=0;i<40&&child.exitCode===null;i++)await sleep(100);if(child.exitCode===null)child.kill();
 await browser?.close().catch(()=>{});await rm(dir,{recursive:true,force:true,maxRetries:10,retryDelay:100});
}
