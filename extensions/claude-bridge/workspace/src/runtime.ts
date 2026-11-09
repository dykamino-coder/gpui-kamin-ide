import fs from 'node:fs'
import path from 'node:path'
import type { ExtensionContext, Webview } from 'vscode'
import { BridgeHost } from '../../extension/src/bridge-host'
import { ipcMain } from '../../extension/src/shim/host-compat'
import { configureEnvironment, withEnvironment, type WorkspaceEnvironment } from './environment.js'
import type { ClientFrame, ClientView, HostAttachment, HostFrame, WorkspaceBackend } from './protocol.js'

export interface SharedBackendOptions {
  root: string
  storage: string
  serverUrl: string
  token: string
  version: string
  environment: WorkspaceEnvironment
}

let started = false

/** Shared handlers have process-wide registries: exactly one runtime per process. */
export function createSharedBackend(options: SharedBackendOptions): WorkspaceBackend & { dispose(): void } {
  if (started) throw new Error('A shared workspace runtime has already been created in this process')
  if (!options.token) throw new Error('A configured workspace token is required')
  const upstream = new URL(options.serverUrl)
  if (!['ws:', 'wss:', 'http:', 'https:'].includes(upstream.protocol) || upstream.username || upstream.password ||
      upstream.search || upstream.hash || !['/', '/ws/session', ''].includes(upstream.pathname)) {
    throw new Error('Expected a bridge server origin or /ws/session URL')
  }
  const root = fs.realpathSync(options.root)
  const storage = path.resolve(options.storage)
  fs.mkdirSync(storage, { recursive: true, mode: 0o700 })
  // Shared handlers also use process.cwd() outside a tab context. This process
  // belongs to one workspace, so give those handlers the same local machine.
  process.chdir(root)
  configureEnvironment(root, options.environment)
  started = true
  const host = new BridgeHost({
    globalStorageUri: { fsPath: storage },
    extension: { packageJSON: { version: options.version } },
  } as ExtensionContext)
  const config = host.getConfigStore()
  config.set({ ...config.get(), serverUrl: options.serverUrl, token: options.token, cwd: root })
  let disposed = false
  const attachments = new Set<HostAttachment>()

  // The shared core exposes desktop server administration helpers with arbitrary
  // HTTP destinations. Keep them closed until the browser's pinned proxy exists.
  const blocked = new Set(['bridge:create-token', 'bridge:server-version', 'bridge:server-fetch'])
  function pinned(value: unknown): Record<string, unknown> {
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Expected connection config')
    return { ...value, serverUrl: options.serverUrl, token: options.token,
      cwd: typeof (value as { cwd?: unknown }).cwd === 'string' ? (value as { cwd: string }).cwd : root }
  }
  function redact(value: unknown): unknown {
    if (typeof value === 'string') return value.split(options.token).join('[workspace credential]')
    if (Array.isArray(value)) return value.map(redact)
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, redact(v)]))
    return value
  }
  function attach(view: ClientView): HostAttachment {
    if (disposed) throw new Error('Workspace runtime is closed')
    let listener: ((frame: ClientFrame) => unknown) | undefined
    let detached = false
    const webview = {
      postMessage: (frame: HostFrame) => detached ? Promise.resolve(false) : view.postMessage(redact(frame) as HostFrame),
      onDidReceiveMessage(callback: (frame: ClientFrame) => unknown) {
        listener = callback
        return { dispose() { listener = undefined } }
      },
    } as unknown as Webview
    const subscription = host.attach(webview)
    host.setWebviewVisible(webview, true)
    const attachment: HostAttachment = {
      async receive(frame) {
        if (disposed || detached) throw new Error('Workspace view is closed')
        if (frame.kind === 'invoke-diagnostic') { await listener?.(frame); return }
        try {
          if (blocked.has(frame.channel)) throw new Error(`Browser adapter is pending for ${frame.channel}`)
          if (frame.kind === 'invoke' && !ipcMain.hasHandler(frame.channel)) throw new Error(`Unsupported workspace invoke: ${frame.channel}`)
          if (frame.kind === 'send' && ipcMain.listenerCount(frame.channel) === 0) throw new Error(`Unsupported workspace send: ${frame.channel}`)
          const args = [...frame.args]
          if (['set-config', 'connect', 'tab:create', 'session:resume'].includes(frame.channel)) args[0] = pinned(args[0])
          await withEnvironment(options.environment, () => listener?.({ ...frame, args }))
        } catch (error) {
          if (frame.kind !== 'invoke') throw error
          await webview.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation,
            ok: false, error: error instanceof Error ? error.message : 'Workspace dispatch failed' })
        }
      },
      dispose() {
        if (detached) return
        detached = true
        subscription.dispose()
        attachments.delete(attachment)
      },
    }
    attachments.add(attachment)
    return attachment
  }
  return {
    attach,
    dispose() {
      if (disposed) return
      disposed = true
      for (const attachment of [...attachments]) attachment.dispose()
      host.dispose()
    },
  }
}
