import { captureRuntime, type RemoteRequire } from './preview-hot'

// The standard web chunk queue is installed before a container's startup
// entries execute. Observe that installation to attach the host runtime before
// a default Refresh preamble or dev-server client can run. No code is rewritten.
export function prepareEntry(source: string, globals: Record<string, unknown>, receive: (require: RemoteRequire) => void): () => void {
  const cleanups: (() => void)[] = []
  const pattern = /(?:self|window|globalThis)\[("(?:\\.|[^"\\])*")\]\s*=\s*(?:self|window|globalThis)\[\1\]\s*\|\|\s*\[\s*\]/g
  const names = new Set(Array.from(source.matchAll(pattern), match => JSON.parse(match[1]) as string))
  for (const name of names) {
    if (!/^(webpack|rspack)Chunk/.test(name) || Object.hasOwn(globals, name)) continue
    const queue: unknown[] = []
    let active = true
    const proxy = new Proxy(queue, { set(target, key, value) {
      Reflect.set(target, key, value)
      if (active && key === 'push' && typeof value === 'function' && value !== Array.prototype.push) captureRuntime(proxy, receive)
      return true
    } })
    Object.defineProperty(globals, name, { configurable: true, enumerable: true,
      get: () => proxy, set(value: unknown) {
        if (value !== proxy) Object.defineProperty(globals, name, { value, configurable: true, writable: true, enumerable: true })
      } })
    cleanups.push(() => {
      active = false
      if (globals[name] === proxy) Object.defineProperty(globals, name, { value: proxy, configurable: true, writable: true, enumerable: true })
    })
  }
  return () => cleanups.forEach(dispose => dispose())
}
