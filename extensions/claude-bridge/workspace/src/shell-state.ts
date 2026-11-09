import fs from 'node:fs'
import path from 'node:path'
import { applyShellOperation, defaultShellState, validateShellState } from '../../webview/shared/workspace-shell.js'
import type { WorkspaceBackend, ClientView } from './protocol.js'

/** Authoritative shell preferences survive browser disconnects and host restarts. */
export function withWorkspaceShell(backend: WorkspaceBackend, file?: string): WorkspaceBackend {
  let state = defaultShellState()
  if (file && fs.existsSync(file)) {
    try { state = validateShellState(JSON.parse(fs.readFileSync(file, 'utf8'))) }
    catch { throw new Error('Workspace shell state is invalid') }
  }
  const views = new Set<ClientView>()
  return { attach(view) {
    const inner = backend.attach(view)
    views.add(view)
    let closed = false
    const snapshot = () => view.postMessage({ kind: 'event', channel: 'workspace:shell:state', args: [state] })
    return {
      async receive(frame) {
        if (closed) throw new Error('Browser view closed')
        if (frame.kind === 'send' && frame.channel === 'workspace:sync') await snapshot()
        if (frame.kind !== 'invoke' || !['workspace:shell:get', 'workspace:shell:update'].includes(frame.channel)) {
          await inner.receive(frame); return
        }
        try {
          if (frame.channel === 'workspace:shell:update') {
            const next = applyShellOperation(state, frame.args[0])
            if (file) {
              fs.mkdirSync(path.dirname(file), { recursive: true })
              fs.writeFileSync(file + '.tmp', JSON.stringify(next), { mode: 0o600 })
              fs.renameSync(file + '.tmp', file)
            }
            state = next
            await Promise.allSettled([...views].map(v => v.postMessage({ kind: 'event', channel: 'workspace:shell:state', args: [state] })))
          }
          if (!closed) await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result: state })
        } catch {
          if (!closed) await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: false, error: 'Workspace layout could not be saved' })
        }
      },
      dispose() { if (!closed) { closed = true; views.delete(view); inner.dispose() } },
    }
  } }
}
