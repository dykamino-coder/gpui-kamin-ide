// Trusted Module Federation remote descriptor; retained by the host with zero pages.
import { createHash } from 'node:crypto'
import { request } from 'node:http'

export interface PreviewDescriptor {
  name: string
  entry: string
  exposedModule: string
  props: Record<string, unknown>
  type: 'global' | 'module'
  revision: number
}
export interface PreviewMount { name?: unknown; entry?: unknown; exposedModule?: unknown; props?: unknown; type?: unknown }

export class WorkspacePreview {
  private descriptor: PreviewDescriptor | null = null
  private upstream?: URL
  private revision = 0
  private forbidden: number[] = []
  private listeners = new Set<() => void>()

  onRemount(listener: () => void): () => void { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }

  blockPorts(ports: number[]): void { this.forbidden = [...new Set([...this.forbidden, ...ports])] }

  mount(input: PreviewMount): PreviewDescriptor {
    if (typeof input.name !== 'string' || !/^[A-Za-z][\w-]{0,63}$/.test(input.name)) throw new Error('Invalid federation remote name')
    if (typeof input.entry !== 'string') throw new Error('Expected a container-local remote entry URL')
    const url = new URL(input.entry)
    const port = Number(url.port || 80)
    if (url.protocol !== 'http:' || !['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname) ||
        url.username || url.password || url.search || url.hash || port < 1024 || port > 65535 || this.forbidden.includes(port)) {
      throw new Error('Remote entry must be HTTP on a container-local dev port, separate from workspace services')
    }
    if (typeof input.exposedModule !== 'string' || !/^\.\/[\w./-]+$/.test(input.exposedModule) || input.exposedModule.includes('..')) {
      throw new Error('Expected an exposed module such as ./Panel')
    }
    if (input.type !== undefined && !['global', 'module'].includes(String(input.type))) throw new Error('Invalid remote entry type')
    const props = this.props(input.props ?? {})
    // publicPath:auto использует проксированный каталог entry. Если dev server
    // уже обслуживает этот prefix, не добавляем его второй раз.
    url.hostname = '127.0.0.1'
    this.upstream = url
    const base = `/preview/remotes/${input.name}`
    this.descriptor = { name: input.name, entry: url.pathname.startsWith(base + '/') ? url.pathname : base + url.pathname,
      exposedModule: input.exposedModule, props, type: input.type === 'module' ? 'module' : 'global', revision: ++this.revision }
    for (const listener of this.listeners) listener()
    return this.copy()!
  }

  update(props: unknown): PreviewDescriptor {
    if (!this.descriptor) throw new Error('No remote is mounted')
    this.descriptor = { ...this.descriptor, props: this.props(props) }
    return this.copy()!
  }

  unmount(): void { this.descriptor = null; this.upstream = undefined; for (const listener of this.listeners) listener() }
  copy(): PreviewDescriptor | null { return this.descriptor ? structuredClone(this.descriptor) : null }

  private props(value: unknown): Record<string, unknown> {
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Expected a JSON props object')
    const json = JSON.stringify(value)
    if (Buffer.byteLength(json) > 64 * 1024) throw new Error('Preview props exceed 64 KiB')
    return JSON.parse(json) as Record<string, unknown>
  }

  target(url: string, transport: 'http' | 'websocket' = 'http'): { port: number; path: string } | undefined {
    if (!this.descriptor || !this.upstream || !url.startsWith(`/preview/remotes/${this.descriptor.name}/`)) return undefined
    const path = url.slice(`/preview/remotes/${this.descriptor.name}`.length)
    if (!path.startsWith('/') || path.startsWith('//') || /[\\\x00-\x1f\x7f]/.test(path)) return undefined
    // Контракт инструмента: HTTP assets относительно entry (publicPath:auto),
    // но HMR server слушает полный /preview/remotes/<name>/ws. Не подменяем
    // его на /ws. HTTP entry с явным prefix сохраняет тот же upstream base.
    const prefixed = this.upstream.pathname.startsWith(`/preview/remotes/${this.descriptor.name}/`)
    return { port: Number(this.upstream.port), path: transport === 'websocket' || prefixed ? url : path }
  }

  async snapshot(): Promise<{ descriptor: PreviewDescriptor | null; build: string | null; available: boolean }> {
    const descriptor = this.copy()
    const upstream = this.upstream
    if (!descriptor || !upstream) return { descriptor, build: null, available: false }
    // Entry fingerprints provide a remount fallback for full rebuilds, without
    // reloading the chat document. Framework HMR remains the primary path.
    const build = await new Promise<string | null>(resolve => {
      const req = request(upstream, { headers: { 'Cache-Control': 'no-cache' } }, response => {
        const hash = createHash('sha256')
        let bytes = 0
        response.on('data', chunk => { bytes += chunk.length; if (bytes > 8 * 1024 * 1024) req.destroy(); else hash.update(chunk) })
        response.on('end', () => resolve(response.statusCode === 200 ? hash.digest('hex') : null))
        response.on('error', () => resolve(null))
      })
      req.setTimeout(1500, () => req.destroy())
      req.on('error', () => resolve(null))
      req.end()
    })
    return { descriptor, build, available: build !== null }
  }
}
