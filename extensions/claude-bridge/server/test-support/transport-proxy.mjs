// Test-only loopback transport fault injection. Never record frames or auth.
import http from 'node:http'
import fs from 'node:fs'
import path from 'node:path'
import { WebSocket, WebSocketServer } from 'ws'
export async function startTransportProxy({ root, targetPort = 3456, port = 3457 }) {
  const sockets = new Set()
  const flag = (name) => path.join(root, name)
  fs.mkdirSync(root, { recursive: true })
  const server = http.createServer((req, res) => {
    const upstream = http.request(
      { hostname: '127.0.0.1', port: targetPort, path: req.url, method: req.method, headers: req.headers },
      (incoming) => {
        res.writeHead(incoming.statusCode, incoming.headers)
        incoming.pipe(res)
      },
    )
    upstream.on('error', () => {
      res.writeHead(502)
      res.end()
    })
    req.pipe(upstream)
  })
  const wss = new WebSocketServer({ server })
  wss.on('connection', (client, req) => {
    const upstream = new WebSocket(`ws://127.0.0.1:${targetPort}${req.url}`)
    const pair = { client, upstream }
    sockets.add(pair)
    const pending = []
    upstream.on('open', () => {
      for (const frame of pending) upstream.send(frame)
      pending.length = 0
    })
    client.on('message', (data, binary) => {
      let type
      try {
        type = JSON.parse(data.toString()).type
      } catch {
        /* non-JSON passes through */
      }
      if (['mcp:response', 'mcp:denied'].includes(type) && fs.existsSync(flag('cut-result'))) {
        // Cut before forwarding: reconnecting client must retain/retry the result.
        fs.rmSync(flag('cut-result'))
        fs.writeFileSync(flag('result-cut'), 'cut before delivery')
        client.terminate()
        upstream.terminate()
        return
      }
      if (upstream.readyState === WebSocket.OPEN) upstream.send(data, { binary })
      else if (upstream.readyState === WebSocket.CONNECTING) pending.push(data)
    })
    upstream.on('message', (data, binary) => {
      if (client.readyState === WebSocket.OPEN) client.send(data, { binary })
    })
    const close = () => {
      sockets.delete(pair)
      client.terminate()
      upstream.terminate()
    }
    client.on('close', close)
    upstream.on('close', close)
    client.on('error', close)
    upstream.on('error', close)
  })
  const timer = setInterval(() => {
    if (!fs.existsSync(flag('disconnect'))) return
    fs.rmSync(flag('disconnect'))
    for (const { client, upstream } of sockets) {
      client.terminate()
      upstream.terminate()
    }
    fs.writeFileSync(flag('disconnected'), 'transport closed')
  }, 20)
  await new Promise((resolve) => server.listen(port, '127.0.0.1', resolve))
  return {
    port: server.address().port,
    close: async () => {
      clearInterval(timer)
      for (const { client, upstream } of sockets) {
        client.terminate()
        upstream.terminate()
      }
      await new Promise((resolve) => wss.close(resolve))
      await new Promise((resolve) => server.close(resolve))
    },
  }
}
if (process.argv[1]?.endsWith('transport-proxy.mjs')) {
  if (!process.argv[2]) throw new Error('Pass an external control directory')
  const proxy = await startTransportProxy({
    root: path.resolve(process.argv[2]),
    targetPort: Number(process.argv[3] ?? 3456),
  })
  console.log(`Loopback fault proxy port=${proxy.port}`)
  process.on('SIGINT', async () => {
    await proxy.close()
    process.exit(0)
  })
}
