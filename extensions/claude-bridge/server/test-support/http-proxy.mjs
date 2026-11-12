// Checkout-only forward proxy standing in for a corporate HTTP(S) proxy.
// It resolves names through a fixed test map only (never DNS), forwards plain
// absolute-form HTTP requests and CONNECT tunnels, and records every target so
// tests can prove which traffic went through the proxy and which did not.
import http from 'node:http'
import net from 'node:net'

/** @param {{ hosts: Record<string, string> }} options  test hostname → "127.0.0.1:<port>" */
export async function startTestProxy(options) {
  /** @type {Array<{ method: string, target: string }>} */
  const seen = []
  const resolve = (host, port) => {
    const mapped = options.hosts[`${host}:${port}`] ?? options.hosts[host]
    if (!mapped) return undefined
    const [h, p] = mapped.split(':')
    return { host: h, port: Number(p ?? port) }
  }
  const server = http.createServer((req, res) => {
    let url
    try {
      url = new URL(req.url ?? '')
    } catch {
      res.writeHead(400).end('absolute-form request required')
      return
    }
    seen.push({ method: req.method ?? 'GET', target: `${url.host}${url.pathname}` })
    const to = resolve(url.hostname, url.port || '80')
    if (!to) {
      res.writeHead(502).end('unknown test host')
      return
    }
    const upstream = http.request(
      { host: to.host, port: to.port, method: req.method, path: url.pathname + url.search, headers: req.headers },
      (up) => {
        res.writeHead(up.statusCode ?? 502, up.headers)
        up.pipe(res)
      },
    )
    upstream.on('error', () => res.writeHead(502).end())
    req.pipe(upstream)
  })
  server.on('connect', (req, socket, head) => {
    seen.push({ method: 'CONNECT', target: req.url ?? '' })
    const [host, port] = (req.url ?? '').split(':')
    const to = resolve(host, port ?? '443')
    if (!to) {
      socket.end('HTTP/1.1 502 Bad Gateway\r\n\r\n')
      return
    }
    const upstream = net.connect(to.port, to.host, () => {
      socket.write('HTTP/1.1 200 Connection Established\r\n\r\n')
      if (head.length) upstream.write(head)
      upstream.pipe(socket)
      socket.pipe(upstream)
    })
    upstream.on('error', () => socket.destroy())
    socket.on('error', () => upstream.destroy())
  })
  await new Promise((r) => server.listen(0, '127.0.0.1', r))
  const port = /** @type {import('node:net').AddressInfo} */ (server.address()).port
  return {
    url: `http://127.0.0.1:${port}`,
    seen,
    close: () =>
      new Promise((r) => {
        server.closeAllConnections?.()
        server.close(() => r(undefined))
      }),
  }
}
