/** Isolated real-process test harness. Node 22+; no npm dependencies. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const scratch = await mkdtemp(join(tmpdir(), 'kinexa-gate1-'));
const children = new Set();
const binary = join(root, 'target/debug', process.platform === 'win32' ? 'kinexa-engine.exe' : 'kinexa-engine');

async function command(args) {
  const child = spawn('cargo', args, { cwd: root, stdio: 'inherit' });
  assert.equal((await once(child, 'exit'))[0], 0, `cargo ${args.join(' ')} failed`);
}

async function freePort() {
  const server = createServer();
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const port = server.address().port;
  await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  return port;
}

async function stop(child) {
  if (child.exitCode !== null) return;
  const exited = once(child, 'exit');
  child.kill('SIGINT');
  const timer = setTimeout(() => child.kill('SIGKILL'), 5000);
  try {
    const [code] = await exited;
    assert.equal(code, 0, 'engine should shut down cleanly');
  } finally {
    clearTimeout(timer);
    children.delete(child);
  }
}

async function start(name, dataName = name) {
  const port = await freePort();
  let yaml = name === 'empty'
    ? 'schema_version: 1\nengine:\n  data_dir: ./data\napi:\n  bind: 127.0.0.1:7700\n'
    : await readFile(join(root, 'config', `${name}.yaml`), 'utf8');
  assert.match(yaml, /^\s+data_dir:/m, 'sample must expose data_dir');
  assert.match(yaml, /^\s+bind:/m, 'sample must expose bind');
  yaml = yaml.replace(/^\s+data_dir:.*$/m, `\n  data_dir: ${JSON.stringify(join(scratch, dataName))}`)
    .replace(/^\s+bind:.*$/m, `\n  bind: 127.0.0.1:${port}`);
  const path = join(scratch, `${name}.yaml`);
  await writeFile(path, yaml);
  const child = spawn(binary, ['--config', path], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  children.add(child);
  let logs = '';
  child.stdout.on('data', data => { logs = (logs + data).slice(-8000); });
  child.stderr.on('data', data => { logs = (logs + data).slice(-8000); });
  const base = `http://127.0.0.1:${port}/api/v1`;
  const api = async (path, method = 'GET', body, headers = {}) => {
    const response = await fetch(`${base}${path}`, {
      method, headers: { ...(body === undefined ? {} : { 'Content-Type': 'application/json' }), ...headers },
      body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(5000),
    });
    return { status: response.status, headers: response.headers, body: await response.json() };
  };
  for (let attempt = 0; ; attempt++) {
    assert.equal(child.exitCode, null, `engine exited during startup:\n${logs}`);
    try {
      const health = await api('/health');
      assert.deepEqual(health.body, { ok: true, data: { status: 'ok' } });
      break;
    } catch (error) {
      if (attempt === 100) throw new Error(`engine did not become healthy:\n${logs}`, { cause: error });
      await delay(50);
    }
  }
  return { child, api, base };
}

async function stream(base, query = 'topics=frame,state,landmarks_2d&max_rate_hz=20') {
  const ws = new WebSocket(`${base.replace('http:', 'ws:')}/stream?${query}`);
  const messages = [];
  ws.addEventListener('message', event => {
    messages.push(JSON.parse(event.data));
    if (messages.length > 2000) messages.shift();
  });
  const until = async (predicate, timeout = 6000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const value = predicate(messages);
      if (value) return value;
      await delay(20);
    }
    throw new Error(`stream condition timed out; message types: ${messages.map(m => m.type).slice(-20)}`);
  };
  await until(list => list[0]);
  assert.equal(messages[0].type, 'hello', 'hello must be first');
  assert.equal(messages[0].protocol_version, 1);
  return { ws, messages, until };
}

export async function cleanup() {
  await Promise.allSettled([...children].map(stop));
  await rm(scratch, { recursive: true, force: true });
}
export { root, scratch, binary, children, command, start, stop, stream };
