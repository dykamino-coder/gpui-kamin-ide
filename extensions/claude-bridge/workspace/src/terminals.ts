import { stat } from 'node:fs/promises'
import { isAbsolute } from 'node:path'
import { PtySessions, pickProfile } from '../../../../src/kamin-host/services/pty-sessions.js'
import { discoverShells } from '../../../../src/kamin-host/services/shells.js'
import type { ClientFrame, ClientView, HostAttachment, WorkspaceBackend } from './protocol.js'

const SENDS = new Set(['pty:write', 'pty:resize', 'pty:kill'])
const INVOKES = new Set(['shells:list', 'pty:spawn'])
const MAX_PER_VIEW = 4
const MAX_TOTAL = 32
const MAX_INPUT = 64 * 1024

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid terminal arguments')
  return value as Record<string, unknown>
}

function dimensions(cols: unknown, rows: unknown): asserts cols is number {
  if (!Number.isInteger(cols) || !Number.isInteger(rows) ||
    (cols as number) < 1 || (cols as number) > 1000 || (rows as number) < 1 || (rows as number) > 1000) {
    throw new Error('Terminal dimensions must be integers between 1 and 1000')
  }
}

/** Environment adapter only: shell discovery, spawn and process lifecycle are shared with KaminIDE. */
export class TerminalWorkspaceBackend implements WorkspaceBackend {
  private readonly attachments = new Set<() => void>()
  private total = 0
  private closed = false

  constructor(private readonly next: WorkspaceBackend, private readonly root: string) {
    if (!isAbsolute(root)) throw new Error('Workspace root must be absolute')
  }

  attach(view: ClientView): HostAttachment {
    if (this.closed) throw new Error('Terminal backend disposed')
    const delegate = this.next.attach(view)
    const owned = new Set<string>()
    let pending = 0
    let disposed = false
    const release = (id: string) => { if (owned.delete(id)) this.total-- }
    const deliver = async (frame: Record<string, unknown>) => {
      try { if (!await view.postMessage(frame)) dispose() } catch { dispose() }
    }
    const sessions = new PtySessions({
      onData: (ptyId, data) => {
        if (owned.has(ptyId) && !disposed) void deliver({ kind: 'event', channel: 'pty:data', args: [{ ptyId, data }] })
      },
      onExit: (ptyId, exitCode, signal) => {
        if (!owned.has(ptyId)) return
        release(ptyId)
        if (!disposed) void deliver({ kind: 'event', channel: 'pty:exit', args: [{ ptyId, exitCode, signal }] })
      },
    })
    const dispose = () => {
      if (disposed) return
      disposed = true
      this.attachments.delete(dispose)
      for (const id of owned) release(id)
      sessions.disposeAllSessions()
      delegate.dispose()
    }
    this.attachments.add(dispose)

    const spawn = async (value: unknown) => {
      const opts = object(value)
      dimensions(opts.cols, opts.rows)
      const cwd = opts.cwd ?? this.root
      if (typeof cwd !== 'string' || !isAbsolute(cwd) || cwd.length > 32768) throw new Error('Terminal cwd must be absolute')
      if (opts.shellId !== undefined && typeof opts.shellId !== 'string') throw new Error('Invalid shell profile')
      if (owned.size + pending >= MAX_PER_VIEW || this.total >= MAX_TOTAL) throw new Error('Terminal session limit reached')
      pending++; this.total++
      let retained = false
      try {
        if (!(await stat(cwd)).isDirectory()) throw new Error('Terminal cwd is not a directory')
        const profiles = await discoverShells()
        if (opts.shellId && !profiles.some(p => p.id === opts.shellId)) throw new Error('Unknown shell profile')
        const profile = pickProfile(profiles, opts.shellId as string | undefined)
        if (!profile) throw new Error('No shell installed')
        if (disposed) throw new Error('Terminal view disposed')
        const ptyId = await sessions.createSession({ cwd, shellId: profile.id, cols: opts.cols as number, rows: opts.rows as number })
        if (disposed) { sessions.disposeSession(ptyId); throw new Error('Terminal view disposed') }
        owned.add(ptyId)
        retained = true
        return { ptyId, profile }
      } finally { pending--; if (!retained) this.total-- }
    }

    return {
      receive: async (frame: ClientFrame) => {
        if (disposed) throw new Error('Terminal view disposed')
        if (frame.kind === 'invoke' && INVOKES.has(frame.channel)) {
          try {
            if (frame.args.length !== (frame.channel === 'pty:spawn' ? 1 : 0)) throw new Error('Invalid terminal argument count')
            const result = frame.channel === 'shells:list' ? await discoverShells() : await spawn(frame.args[0])
            await deliver({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result })
          } catch (error) {
            if (!disposed) await deliver({ kind: 'invoke-reply', id: frame.id, generation: frame.generation,
              ok: false, error: error instanceof Error ? error.message : 'Terminal operation failed' })
          }
          return
        }
        if (frame.kind !== 'send' || !SENDS.has(frame.channel)) return delegate.receive(frame)
        if (frame.args.length !== 1) throw new Error('Invalid terminal argument count')
        const opts = object(frame.args[0])
        if (typeof opts.ptyId !== 'string' || !owned.has(opts.ptyId)) throw new Error('Terminal is not owned by this view')
        if (frame.channel === 'pty:write') {
          if (typeof opts.data !== 'string' || Buffer.byteLength(opts.data) > MAX_INPUT) throw new Error('Terminal input exceeds limit')
          sessions.writeToSession(opts.ptyId, opts.data)
        } else if (frame.channel === 'pty:resize') {
          dimensions(opts.cols, opts.rows)
          sessions.resizeSession(opts.ptyId, opts.cols as number, opts.rows as number)
        } else sessions.disposeSession(opts.ptyId)
      },
      dispose,
    }
  }

  dispose(): void {
    this.closed = true
    for (const dispose of this.attachments) dispose()
  }
}
