import { AsyncLocalStorage } from 'node:async_hooks'
import { randomUUID } from 'node:crypto'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { saveAttachment } from '../../extension/src/attachments.js'
import { relayToolUsage } from '../../extension/src/tool-usage-relay.js'
import type { WorkspaceEnvironment } from './environment.js'
import type { ClientView, InvokeFrame, HostFrame, WorkspaceBackend } from './protocol.js'

const MAX_BYTES = 8 * 1024 * 1024
const EFFECT = 'workspace:effect'
const RESULT = 'workspace:effect-result'
interface Request {
  effect(operation: string, payload: unknown): Promise<unknown>
  downloads: string[]
}

/** Environment effects have view/request ownership, independent of session lifetime. */
export function createBrowserEnvironment(options: { root: string; timeoutMs?: number; preview?: WorkspaceEnvironment['preview'] }) {
  const root = path.resolve(options.root)
  const requests = new AsyncLocalStorage<Request>()
  const viewers = new Set<Request['effect']>()
  function request(): Request {
    const current = requests.getStore()
    if (!current) throw new Error('Browser effect requires an originating view')
    return current
  }
  const effect = (operation: string, payload: unknown) => request().effect(operation, payload)
  async function stage(): Promise<string> {
    const current = request()
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'bridgeweb-download-'))
    current.downloads.push(directory)
    return directory
  }
  async function upload(value: unknown): Promise<string> {
    if (!value || typeof value !== 'object') throw new Error('Invalid browser upload')
    const { name, base64 } = value as Record<string, unknown>
    if (typeof name !== 'string' || typeof base64 !== 'string' || base64.length > Math.ceil(MAX_BYTES / 3) * 4) {
      throw new Error('Upload exceeds the 8 MiB per-file limit')
    }
    if (Buffer.byteLength(base64, 'base64') > MAX_BYTES) throw new Error('Upload exceeds the 8 MiB per-file limit')
    return saveAttachment(base64, name, path.join(root, '.bridge-uploads'))
  }
  const unavailable = async (): Promise<never> => { throw new Error('KaminIDE host feature, not Bridge') }
  const environment: WorkspaceEnvironment = {
    preview: options.preview,
    clipboard: {
      async readText() {
        const value = await effect('clipboard-read', {})
        if (typeof value !== 'string') throw new Error('Invalid clipboard result')
        return value
      },
      async writeText(text) { await effect('clipboard-write', { text }) },
    },
    async message(level, text, actions) {
      // Notifications may originate from background host work with no request.
      // They are best effort; closing every page does not fail that work.
      const target = requests.getStore()?.effect ?? viewers.values().next().value
      if (!target) return undefined
      const result = await target('message', { level, text, actions }).catch(() => undefined)
      return typeof result === 'string' && actions.includes(result) ? result : undefined
    },
    async pickPaths(picker) {
      if (picker.canSelectFolders) {
        // Shared bulk-export writes its files into this owned staging directory.
        if (picker.title === 'Choose a folder for the exported session logs') return [await stage()]
        const result = await effect('container-folder', { title: picker.title, defaultPath: root })
        if (result === null) return undefined
        if (typeof result !== 'string' || !path.isAbsolute(result) || !(await fs.stat(result)).isDirectory()) {
          throw new Error('Choose an existing absolute container directory')
        }
        return [result]
      }
      const files = await effect('upload', { multiple: picker.canSelectMany === true, filters: picker.filters, maxBytes: MAX_BYTES })
      if (files === null) return undefined
      if (!Array.isArray(files) || files.length > 32 || (!picker.canSelectMany && files.length > 1)) throw new Error('Invalid upload selection')
      const paths: string[] = []
      for (const file of files) paths.push(await upload(file))
      return paths
    },
    async savePath(picker) {
      const uri = picker.defaultUri as { fsPath?: unknown } | undefined
      const name = typeof uri?.fsPath === 'string' ? path.basename(uri.fsPath) : 'download'
      return path.join(await stage(), name === '.' || name === '..' ? 'download' : name)
    },
    async openExternal(url) {
      const target = new URL(url)
      if (!['https:', 'http:', 'mailto:'].includes(target.protocol) || target.username || target.password) throw new Error('Unsupported external URL')
      return await effect('open-external', { url: target.href }) === true
    },
    openDocument: unavailable,
    reveal: unavailable,
  }

  function wrap(backend: WorkspaceBackend): WorkspaceBackend {
    const views = new Set<ClientView>()
    return { attach(view) {
      let closed = false
      const pending = new Map<string, { resolve(value: unknown): void; reject(error: Error): void; timer: ReturnType<typeof setTimeout> }>()
      const contexts = new Map<string, Request>()
      async function effect(operation: string, payload: unknown): Promise<unknown> {
        if (closed) throw new Error('Browser view closed')
        if (pending.size >= 32) throw new Error('Too many pending browser effects')
        const id = randomUUID()
        return new Promise((resolve, reject) => {
          const timer = setTimeout(() => { pending.delete(id); reject(new Error('Browser effect timed out')) }, options.timeoutMs ?? 120_000)
          pending.set(id, { resolve, reject, timer })
          void view.postMessage({ kind: 'event', channel: EFFECT, args: [{ id, operation, payload }] }).then(sent => {
            if (!sent) settle(id, undefined, 'Browser view unavailable')
          }, () => settle(id, undefined, 'Browser view unavailable'))
        })
      }
      function settle(id: string, value: unknown, error?: string) {
        const item = pending.get(id)
        if (!item) return
        pending.delete(id)
        clearTimeout(item.timer)
        if (error) item.reject(new Error(error))
        else item.resolve(value)
      }
      async function flush(context: Request, deliver: boolean) {
        for (const directory of context.downloads.splice(0)) {
          try {
            for (const name of await fs.readdir(directory)) {
              const file = path.join(directory, name)
              const stat = await fs.lstat(file)
              if (!stat.isFile()) throw new Error('Invalid export file')
              if (deliver) {
                if (stat.size > MAX_BYTES) throw new Error('Download exceeds the 8 MiB per-file limit')
                if (await effect('download', { name, base64: (await fs.readFile(file)).toString('base64') }) !== true) {
                  throw new Error('Download was not accepted by the browser')
                }
              }
              await fs.unlink(file)
            }
          } finally {
            // Only exact files in a directory created by this request are removed.
            for (const name of await fs.readdir(directory)) {
              const file = path.join(directory, name)
              if ((await fs.lstat(file)).isFile()) await fs.unlink(file)
            }
            await fs.rmdir(directory)
          }
        }
      }
      const key = (frame: HostFrame | InvokeFrame) => `${frame.generation}:${frame.id}`
      const inner = backend.attach({ async postMessage(frame) {
        if (frame.kind === 'invoke-reply') {
          const context = contexts.get(key(frame))
          if (context) {
            contexts.delete(key(frame))
            try { await flush(context, frame.ok === true) }
            catch (error) { return view.postMessage({ ...frame, ok: false, error: error instanceof Error ? error.message : 'Download failed' }) }
          }
        }
        return view.postMessage(frame)
      } })
      viewers.add(effect)
      views.add(view)
      return {
        async receive(frame) {
          if (closed) throw new Error('Browser view closed')
          if (frame.kind === 'send' && frame.channel === 'toolusage:open') {
            relayToolUsage(frame.args[0], (channel, name) => {
              for (const target of views) void target.postMessage({ kind: 'event', channel, args: [name] }).catch(() => false)
            })
            return
          }
          if (frame.kind === 'send' && frame.channel === RESULT) {
            const [id, value, error] = frame.args
            if (typeof id !== 'string' || (error !== undefined && typeof error !== 'string')) throw new Error('Invalid browser effect result')
            settle(id, value, error)
            return
          }
          const context: Request = { effect, downloads: [] }
          if (frame.kind === 'invoke') contexts.set(key(frame), context)
          try {
            await requests.run(context, async () => {
              if (frame.kind === 'invoke' && ['save-dropped-file', 'save-clipboard-image'].includes(frame.channel)) {
                try {
                  const name = frame.channel === 'save-clipboard-image' ? (frame.args[1] === 'image/jpeg' ? 'pasted-image.jpg' : 'pasted-image.png') : frame.args[1]
                  const result = await upload({ name, base64: frame.args[0] })
                  await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result })
                } catch (error) {
                  await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: false, error: error instanceof Error ? error.message : 'Upload failed' })
                }
              } else await inner.receive(frame)
            })
          } finally {
            if (frame.kind === 'invoke') contexts.delete(key(frame))
            await flush(context, false)
          }
        },
        dispose() {
          if (closed) return
          closed = true
          viewers.delete(effect)
          views.delete(view)
          for (const id of pending.keys()) settle(id, undefined, 'Browser view closed')
          inner.dispose()
        },
      }
    } }
  }
  return { environment, wrap }
}
