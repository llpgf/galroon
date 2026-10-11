// A local stand-in for api.vndb.org/kana: forwards to the real VNDB, or injects outages on demand.
import { createServer, type Server } from 'node:http';

export type Mode = 'proxy' | 'timeout' | 'refused' | '429' | 'flaky';

export class FaultyVndb {
  mode: Mode = 'proxy';
  delayMs = 0;
  requests = 0;
  private server!: Server;
  port = 0;
  get url() { return `http://127.0.0.1:${this.port}/kana`; }

  async start() {
    this.server = createServer(async (req, res) => {
      this.requests++;
      if (this.mode === 'refused') { req.socket.destroy(); return; }
      if (this.mode === 'timeout') return; // never answer; Core gives up after its own timeout
      if (this.mode === '429' || (this.mode === 'flaky' && this.requests % 2 === 0)) { res.writeHead(this.mode === '429' ? 429 : 502).end('{"error":"injected"}'); return; }
      const chunks: Buffer[] = [];
      for await (const c of req) chunks.push(c as Buffer);
      if (this.delayMs) await new Promise(r => setTimeout(r, this.delayMs));
      try {
        const upstream = await fetch(`https://api.vndb.org${req.url}`, { method: req.method, headers: { 'Content-Type': 'application/json' }, body: Buffer.concat(chunks) });
        res.writeHead(upstream.status, { 'Content-Type': 'application/json' }).end(Buffer.from(await upstream.arrayBuffer()));
      } catch { res.writeHead(502).end(); }
    });
    await new Promise<void>(ok => this.server.listen(0, '127.0.0.1', ok));
    this.port = (this.server.address() as { port: number }).port;
    return this;
  }
  async stop() { this.server.closeAllConnections(); await new Promise(ok => this.server.close(ok)); }
}
