// Test-only native external window. The shipping app never starts or depends on this helper.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import path from 'node:path';
export async function focusTarget() {
  const child = spawn(path.resolve('target/debug/examples/focus-target.exe'), [], { stdio: 'pipe' });
  let error = ''; const queue = [], waiting = [];
  child.stderr.on('data', b => error += b.toString());
  const lines = createInterface({ input: child.stdout });
  lines.on('line', line => { const next = waiting.shift(); if (next) next.resolve(line); else queue.push(line); });
  child.on('error', e => { error = `${e.message}. Build with cargo build -p opentoken --example focus-target`; for (const next of waiting.splice(0)) next.reject(Error(error)); });
  function next() {
    if (queue.length) return Promise.resolve(queue.shift());
    return new Promise((resolve, reject) => {
      const entry = { resolve: value => { clearTimeout(timer); resolve(value); }, reject: e => { clearTimeout(timer); reject(e); } };
      const timer = setTimeout(() => { const i = waiting.indexOf(entry); if (i >= 0) waiting.splice(i, 1); reject(Error(`Native focus helper timed out. ${error}`)); }, 8000);
      waiting.push(entry);
    });
  }
  async function request(command, expected) {
    child.stdin.write(`${command}\n`); const response = await next();
    if (response !== expected) throw Error(`Focus helper ${command}: ${response}. ${error}`);
  }
  if (await next() !== 'ready') throw Error(`Native focus helper did not initialize. ${error}`);
  return {
    backdrop: pid => request(`backdrop ${pid}`, 'backdrop'),
    color: rgb => request(`color ${((rgb & 0xff) << 16 | rgb & 0xff00 | rgb >>> 16).toString(16)}`, 'painted'),
    capture: async (pid, output) => {
      const snapshot = spawn(path.resolve('target/debug/examples/focus-target.exe'), ['--capture', String(pid), output], { stdio: 'pipe' });
      let errors = ''; snapshot.stderr.on('data', b => errors += b.toString());
      const code = await new Promise((resolve, reject) => { snapshot.on('error', reject); snapshot.on('exit', resolve); });
      if (code !== 0) throw Error(`Native fixture capture failed: ${errors}`);
    },
    focus: () => request('focus', 'focused'),
    hide: () => request('hide', 'hidden'),
    park: () => request('park', 'parked'),
    allow: pid => request(`allow ${pid}`, 'allowed'),
    main: pid => request(`main ${pid}`, 'main'),
    escape: pid => request(`escape ${pid}`, 'escaped'),
    confirmCliFixture: pid => request(`cliyes ${pid}`, 'cli-confirmed'),
    trayDown: (pid, index = 0) => request(`traydown ${pid} ${index}`, 'trayed'),
    trayUp: (pid, index = 0) => request(`trayup ${pid} ${index}`, 'trayed'),
    trayDouble: (pid, index = 0) => request(`traydouble ${pid} ${index}`, 'trayed'),
    trayDrop: (pid, index = 0) => request(`traydrop ${pid} ${index}`, 'trayed'),
    close: async () => {
      if (child.exitCode === null) {
        child.stdin.end('quit\n');
        await Promise.race([new Promise(r => child.once('exit', r)), new Promise(r => setTimeout(r, 1500))]);
        if (child.exitCode === null) child.kill();
      }
      lines.close();
    },
  };
}
