#!/usr/bin/env node
// kamin-probe CLI-клиент (plan/96 §0). JSON-lines по TCP.
// Использование:
//   node scripts/probe.mjs ping
//   node scripts/probe.mjs tree
//   node scripts/probe.mjs '{"cmd":"metric","id":"status-bar"}'
// Порт: KAMIN_PROBE_PORT (default 9333). Токен: KAMIN_PROBE_TOKEN.

import net from 'node:net';

const port = Number(process.env.KAMIN_PROBE_PORT ?? 9333);
const token = process.env.KAMIN_PROBE_TOKEN;

const arg = process.argv[2];
if (!arg) {
  console.error('usage: probe.mjs <cmd | json-request>');
  process.exit(2);
}
const req = arg.trim().startsWith('{') ? JSON.parse(arg) : { cmd: arg };
if (token) req.token = token;

const sock = net.connect({ host: '127.0.0.1', port }, () => {
  sock.write(JSON.stringify(req) + '\n');
});
sock.setTimeout(10_000, () => {
  console.error('probe: timeout');
  sock.destroy();
  process.exit(1);
});

let buf = '';
sock.on('data', (chunk) => {
  buf += chunk;
  const nl = buf.indexOf('\n');
  if (nl === -1) return;
  const line = buf.slice(0, nl);
  try {
    const json = JSON.parse(line);
    console.log(JSON.stringify(json, null, 2));
    process.exit(json.ok ? 0 : 1);
  } catch {
    console.error('probe: bad response:', line);
    process.exit(1);
  }
});
sock.on('error', (e) => {
  console.error('probe: ' + e.message);
  process.exit(1);
});
