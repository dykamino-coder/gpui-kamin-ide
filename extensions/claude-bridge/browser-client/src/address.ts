// The one place where an address of the workspace host is built. The host
// serves fixed paths (`/bridge`, `/preview/state`, `/preview/remotes/<name>/…`);
// a gateway may publish them under a prefix, and HTTP and WebSocket routes may
// have different prefixes or hosts.

/**
 * Where the paths of the workspace host are published. A host path `/x` is
 * addressed as `<http>/x` over HTTP and as `<ws>/x` over WebSocket; the gateway
 * removes the prefix and forwards `/x` unchanged. Each member is a URL or a
 * path resolved against the page (or against `origin` of the client). Without
 * a member the root of that origin is used, as before.
 */
export interface HostBase {
  /** Base of HTTP paths, e.g. `/gateway/files` or `https://files.example/ws-1`. */
  http?: string | URL
  /** Base of WebSocket paths, e.g. `/gateway/live` or `wss://live.example/ws-1`. `http(s):` is read as `ws(s):`. */
  ws?: string | URL
}

export interface HostAddresses {
  /** Address of a host HTTP path such as `/preview/state`. */
  http(path: string): URL
  /** Address of a host WebSocket path such as `/bridge`. */
  ws(path: string): URL
}

export function hostAddresses(base: HostBase = {}, origin: string | URL = location.href): HostAddresses {
  const root = (value: string | URL | undefined, socket: boolean): URL => {
    const url = new URL(value ?? '/', origin)
    if (!/^(https?|wss?):$/.test(url.protocol) || (!socket && url.protocol.startsWith('ws'))) throw new Error('Unsupported host base')
    if (socket) url.protocol = url.protocol === 'https:' || url.protocol === 'wss:' ? 'wss:' : 'ws:'
    url.search = ''
    url.hash = ''
    if (!url.pathname.endsWith('/')) url.pathname += '/'
    return url
  }
  const under = (base: URL, path: string): URL => {
    // The path is appended, never resolved as a reference: `..`, a scheme or a
    // second slash cannot move the address outside its base.
    const url = path.startsWith('/') ? new URL(path.slice(1), base) : undefined
    if (!url || !url.href.startsWith(base.href)) throw new Error('Host path leaves its base')
    return url
  }
  const http = root(base.http, false), ws = root(base.ws, true)
  return { http: path => under(http, path), ws: path => under(ws, path) }
}
