// Starts an isolated Galroon Core (crates/core/examples/ui_core.rs) for one test.
import { spawn, type ChildProcess } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, realpathSync, rmSync } from 'node:fs';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = resolve(fileURLToPath(new URL('.', import.meta.url)), '../../..');
export const RUNS = join(ROOT, 'test-output/e2e/runs');
export const SNAPSHOTS = join(ROOT, 'test-output/e2e/snapshots');
export const SANDBOX = process.env.GALROON_SANDBOX || '/Volumes/新增磁碟區/sandbox_data';
const BINARY = join(ROOT, 'target/debug/examples/ui_core');

export type Session = { url: string; token: string; library_id: string; device_id: string; instance_id: string; api_min: number; api_max: number };

async function freePort(): Promise<number> {
  return new Promise((ok, fail) => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const p = (s.address() as { port: number }).port; s.close(() => ok(p)); }); s.on('error', fail); });
}

export class Core {
  proc: ChildProcess; dir: string; session: Session; env: Record<string, string>;
  constructor(proc: ChildProcess, dir: string, session: Session, env: Record<string, string>) { this.proc = proc; this.dir = dir; this.session = session; this.env = env; }
  get url() { return this.session.url; }
  headers() { return { Authorization: `Bearer ${this.session.token}`, 'X-Galroon-Library': this.session.library_id, 'X-Galroon-Device': this.session.device_id, 'Content-Type': 'application/json' }; }
  async api<T = any>(path: string, body?: unknown, method?: string): Promise<T> {
    const response = await fetch(`${this.url}/api${path}`, { method: method || (body === undefined ? 'GET' : 'POST'), headers: this.headers(), body: body === undefined ? undefined : JSON.stringify(body) });
    const text = await response.text();
    if (!response.ok) throw new Error(`${method || (body === undefined ? 'GET' : 'POST')} ${path} → ${response.status}: ${text}`);
    return text ? JSON.parse(text) : (undefined as T);
  }
  /** Graceful stop; data directory stays for snapshots. */
  async stop() { await this.signal('SIGTERM'); }
  /** Simulates a crash or forced close. */
  async kill() { await this.signal('SIGKILL'); }
  private async signal(sig: NodeJS.Signals) {
    if (this.proc.exitCode !== null || this.proc.signalCode) return;
    const exited = new Promise(ok => this.proc.once('exit', ok));
    this.proc.kill(sig);
    await exited;
  }
  /** Restart on the same data directory (same port is not guaranteed). */
  async restart(env: Record<string, string> = this.env) { await this.stop(); return startCore({ dir: this.dir, env, fresh: false }); }
}

export async function startCore(options: { name?: string; dir?: string; from?: string; env?: Record<string, string>; fresh?: boolean } = {}): Promise<Core> {
  if (!existsSync(BINARY)) throw new Error(`Build Core first: cargo build -p galroon-core --example ui_core (${BINARY} missing)`);
  const dir = options.dir || join(RUNS, `${options.name || 'core'}-${Date.now()}`);
  if (options.fresh !== false && !options.dir) { rmSync(dir, { recursive: true, force: true }); mkdirSync(dir, { recursive: true }); }
  if (options.from) cpSync(options.from, dir, { recursive: true });
  mkdirSync(dir, { recursive: true });
  const real = realpathSync(dir);
  const port = await freePort();
  const env = options.env || {};
  const proc = spawn(BINARY, [real, String(port)], { env: { ...process.env, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  let log = '';
  proc.stderr!.on('data', d => { log += d; if (log.length > 200_000) log = log.slice(-100_000); });
  const session = await new Promise<Session>((ok, fail) => {
    let out = '';
    const timer = setTimeout(() => fail(new Error(`Core did not start: ${log}`)), 30_000);
    proc.stdout!.on('data', d => { out += d; const line = out.split('\n').find(l => l.trim().startsWith('{')); if (line) { clearTimeout(timer); ok(JSON.parse(line)); } });
    proc.once('exit', code => { clearTimeout(timer); fail(new Error(`Core exited ${code}: ${log}`)); });
  });
  return new Core(proc, real, session, env);
}

export function snapshot(core: Core, name: string) {
  const target = join(SNAPSHOTS, name);
  rmSync(target, { recursive: true, force: true });
  cpSync(core.dir, target, { recursive: true });
  return target;
}

export async function waitFor<T>(what: string, probe: () => Promise<T | undefined | false | null>, timeoutMs = 600_000, everyMs = 2_000): Promise<T> {
  const until = Date.now() + timeoutMs;
  let last: unknown;
  while (Date.now() < until) {
    try { const value = await probe(); if (value) return value; } catch (e) { last = e; }
    await new Promise(r => setTimeout(r, everyMs));
  }
  throw new Error(`Timed out waiting for ${what}${last ? `: ${last}` : ''}`);
}
