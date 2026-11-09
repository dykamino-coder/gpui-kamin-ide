import type { KaminBridgeApi } from '../../shared/types'
import type { WebviewApi } from '../lib/webview-api'

interface Screen {
  ptyId: string; profile: { id: string }; cols: number; rows: number
  screen: string; sequence: number; exitCode?: number; truncated?: boolean; outputError?: string
}
interface Options {
  api: WebviewApi
  bridge: Pick<KaminBridgeApi, 'ptySpawn' | 'ptyKill'>
  tabId: string
  cwd: string
  terminal: { cols: number; rows: number; reset(): void; resize(cols: number, rows: number): void; write(data: string, callback?: () => void): void }
  onIdentity(ptyId: string | null): void
  onProfile(shellId: string): void
}

/** Disposable projection of the host's terminal. Only explicit UI actions mutate it. */
export function attachWorkspaceTerminal(options: Options): { selectShell(shellId: string): void; dispose(): void } {
  const { api, bridge, terminal, tabId } = options
  let descriptor: string | undefined
  let sequence = -1
  let ready = false, disposed = false, offline = false, canCreate = true, spawning = false
  let rendering = Promise.resolve()
  const write = (data: string) => new Promise<void>(resolve => terminal.write(data, resolve))
  function render(operation: () => Promise<void>): void {
    rendering = rendering.then(async () => { if (!disposed) await operation() }).catch(() => {
      ready = false; options.onIdentity(null)
    })
  }
  const report = (message: string) => render(() => write(`\r\n[${message}]\r\n`))
  function sync(): void {
    try { api.postMessage({ kind: 'send', channel: 'workspace:terminal-sync', args: [tabId] }) }
    catch (error) { report(String(error)) }
  }
  function spawn(shellId?: string): void {
    if (spawning) { report('Terminal operation already pending'); return }
    spawning = true
    ready = false
    options.onIdentity(null)
    void bridge.ptySpawn({ tabId, shellId, cwd: options.cwd, cols: Math.max(20, terminal.cols), rows: Math.max(5, terminal.rows) })
      .then(() => { if (!disposed && !offline) sync() })
      .catch(error => { if (!disposed) report(`Terminal outcome may be unknown: ${String(error)}`) })
      .finally(() => { spawning = false })
  }
  const offMessage = api.onHostMessage!((frame: unknown) => {
    const f = frame as { kind?: string; channel?: string; args?: unknown[] } | null
    if (disposed || f?.kind !== 'event') return
    if (f.channel === 'bridge:reconnected') { offline = false; sync(); return }
    if (f.channel === 'workspace:terminal-state' && f.args?.[1] === tabId) {
      if (offline) return
      const snapshot = f.args[0] as Screen | null
      if (!snapshot) {
        descriptor = undefined; ready = false; options.onIdentity(null)
        if (canCreate) { canCreate = false; spawn() }
        return
      }
      canCreate = false
      if (descriptor === snapshot.ptyId && snapshot.sequence < sequence) return
      ready = false
      options.onIdentity(null)
      descriptor = snapshot.ptyId
      sequence = snapshot.sequence
      ready = snapshot.exitCode === undefined && !snapshot.outputError
      render(async () => {
        // xterm.reset does not flush pending writes. Apply the snapshot only
        // after prior parser callbacks, then queue later deltas behind it.
        options.onIdentity(null)
        terminal.reset()
        terminal.resize(snapshot.cols, snapshot.rows)
        await write(snapshot.screen)
        if (snapshot.truncated) await write('\r\n[Older host scrollback omitted by snapshot size limit]\r\n')
        if (snapshot.outputError) await write(`\r\n[${snapshot.outputError}]\r\n`)
        if (snapshot.exitCode !== undefined) await write(`\r\n[shell exited: ${snapshot.exitCode}]\r\n`)
        if (!disposed) {
          options.onProfile(snapshot.profile.id)
          options.onIdentity(ready && !offline && descriptor === snapshot.ptyId ? descriptor : null)
        }
      })
    } else if (f.channel === 'pty:data' || f.channel === 'pty:exit') {
      const event = f.args?.[0] as { ptyId: string; data?: string; sequence: number; exitCode?: number; outputError?: string } | undefined
      if (!ready || offline || !event || event.ptyId !== descriptor || event.sequence <= sequence) return
      sequence = event.sequence
      if (f.channel === 'pty:data') render(() => write(event.data!))
      else {
        ready = false; options.onIdentity(null)
        if (event.outputError) report(event.outputError)
        report(`shell exited: ${event.exitCode}`)
      }
    } else if (f.channel === 'workspace:terminal-removed') {
      const event = f.args?.[0] as { ptyId: string; tabId: string } | undefined
      if (event?.tabId !== tabId || event.ptyId !== descriptor) return
      descriptor = undefined; ready = false; options.onIdentity(null)
    }
  })
  const offDisconnect = api.onDisconnect!(() => {
    offline = true; canCreate = false; ready = false; options.onIdentity(null)
    report('Connection lost; terminal continues on workspace host')
  })
  sync()
  return {
    selectShell(shellId) {
      if (disposed || offline || spawning) { report('Terminal unavailable or operation pending'); return }
      try {
        if (descriptor) bridge.ptyKill(descriptor)
        descriptor = undefined
        canCreate = false
        spawn(shellId)
      } catch (error) { report(String(error)) }
    },
    dispose() {
      disposed = true
      offMessage(); offDisconnect()
      options.onIdentity(null)
    },
  }
}
