import { randomUUID } from 'node:crypto'
import { stat } from 'node:fs/promises'
import { isAbsolute } from 'node:path'
import { PtySessions, pickProfile } from '../../../../src/kamin-host/services/pty-sessions.js'
import { discoverShells, type ShellProfile } from '../../../../src/kamin-host/services/shells.js'
import { TerminalScreen, SCROLLBACK } from './terminal-screen.js'
import type { ClientFrame, ClientView, HostAttachment, WorkspaceBackend } from './protocol.js'

const SENDS = new Set(['pty:write', 'pty:resize', 'pty:kill', 'workspace:terminal-sync'])
const INVOKES = new Set(['shells:list', 'pty:spawn'])
const MAX_TOTAL = 32, MAX_CELLS = 8_000_000, MAX_INPUT = 64 * 1024
interface TerminalRecord {
  ptyId: string; tabId: string; profile: ShellProfile; cwd: string; cols: number; rows: number
  screen: TerminalScreen; sequence: number; reserved: number; exitCode?: number; outputError?: string
}
interface SpawnJob { cancelled: boolean; promise: Promise<{ ptyId: string; profile: ShellProfile }> }
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid terminal arguments')
  return value as Record<string, unknown>
}
function dimensions(cols: unknown, rows: unknown): asserts cols is number {
  if (!Number.isInteger(cols) || !Number.isInteger(rows) ||
    (cols as number) < 1 || (cols as number) > 500 || (rows as number) < 1 || (rows as number) > 200) {
    throw new Error('Terminal dimensions must be integers within 500 columns and 200 rows')
  }
}
function identity(value: unknown): string {
  if (typeof value !== 'string' || !value || value.length > 256) throw new Error('Invalid terminal tab identity')
  return value
}
function cells(cols: number, rows: number): number { return cols * (rows + SCROLLBACK) }

/** Shared execution, with process-owned presentation; authenticated pages are observers. */
export class TerminalWorkspaceBackend implements WorkspaceBackend {
  private readonly attachments = new Set<{ deliver(frame: Record<string, unknown>): Promise<void>; dispose(): void }>()
  private readonly records = new Map<string, TerminalRecord>()
  private readonly pending = new Map<string, SpawnJob>()
  private readonly sessions: PtySessions
  private reservedCells = 0
  private closed = false

  constructor(private readonly next: WorkspaceBackend, private readonly root: string) {
    if (!isAbsolute(root)) throw new Error('Workspace root must be absolute')
    this.sessions = new PtySessions({
      onData: (ptyId, data) => {
        const record = this.records.get(ptyId)
        record?.screen.write(data, () => this.broadcast('pty:data', { ptyId, data, sequence: ++record.sequence }))
      },
      onExit: (ptyId, exitCode, signal) => {
        const record = this.records.get(ptyId)
        if (record) void record.screen.order(() => {
          if (record.exitCode !== undefined) return
          record.exitCode = exitCode
          this.broadcast('pty:exit', { ptyId, exitCode, signal, sequence: ++record.sequence, outputError: record.outputError })
        })
      },
    })
  }

  private broadcast(channel: string, ...args: unknown[]): void {
    for (const view of this.attachments) void view.deliver({ kind: 'event', channel, args })
  }

  private remove(record: TerminalRecord): void {
    this.records.delete(record.ptyId)
    this.reservedCells -= record.reserved
    this.sessions.disposeSession(record.ptyId)
    record.screen.dispose()
    this.broadcast('pty:exit', { ptyId: record.ptyId, exitCode: record.exitCode ?? -1, sequence: ++record.sequence })
    this.broadcast('workspace:terminal-removed', { ptyId: record.ptyId, tabId: record.tabId })
  }

  private end(tabId?: string): void {
    for (const [id, job] of this.pending) if (!tabId || id === tabId) job.cancelled = true
    for (const record of [...this.records.values()]) if (!tabId || record.tabId === tabId) this.remove(record)
  }

  private spawn(value: unknown): Promise<{ ptyId: string; profile: ShellProfile }> {
    const opts = object(value)
    dimensions(opts.cols, opts.rows)
    const cwd = opts.cwd ?? this.root
    if (typeof cwd !== 'string' || !isAbsolute(cwd) || cwd.length > 32768) throw new Error('Terminal cwd must be absolute')
    if (opts.shellId !== undefined && typeof opts.shellId !== 'string') throw new Error('Invalid shell profile')
    const tabId = opts.tabId === undefined ? randomUUID() : identity(opts.tabId)
    const existing = [...this.records.values()].find(r => r.tabId === tabId)
    if (existing) return Promise.resolve({ ptyId: existing.ptyId, profile: existing.profile })
    const prior = this.pending.get(tabId)
    if (prior) return prior.promise
    const cols = opts.cols as number, rows = opts.rows as number, reserved = cells(cols, rows)
    if (this.records.size + this.pending.size >= MAX_TOTAL || this.reservedCells + reserved > MAX_CELLS) {
      throw new Error('Terminal session resource limit reached')
    }
    this.reservedCells += reserved
    const job: SpawnJob = { cancelled: false, promise: undefined! }
    this.pending.set(tabId, job)
    job.promise = (async () => {
      let retained = false
      try {
        if (!(await stat(cwd)).isDirectory()) throw new Error('Terminal cwd is not a directory')
        const profiles = await discoverShells()
        if (opts.shellId && !profiles.some(p => p.id === opts.shellId)) throw new Error('Unknown shell profile')
        const profile = pickProfile(profiles, opts.shellId as string | undefined)
        if (!profile) throw new Error('No shell installed')
        if (this.closed || job.cancelled) throw new Error('Terminal spawn cancelled')
        const ptyId = await this.sessions.createSession({ cwd, shellId: profile.id, cols, rows })
        if (this.closed || job.cancelled) { this.sessions.disposeSession(ptyId); throw new Error('Terminal spawn cancelled') }
        let screen: TerminalScreen
        try { screen = new TerminalScreen(cols, rows, () => {
          const record = this.records.get(ptyId)
          if (record) {
            record.outputError = 'Terminal parser input limit exceeded'
            this.sessions.disposeSession(ptyId)
          }
        }, data => this.sessions.writeToSession(ptyId, data)) } catch (error) {
          this.sessions.disposeSession(ptyId)
          throw error
        }
        this.records.set(ptyId, { ptyId, tabId, profile, cwd, cols, rows, screen, sequence: 0, reserved })
        retained = true
        return { ptyId, profile }
      } finally {
        this.pending.delete(tabId)
        if (!retained) this.reservedCells -= reserved
      }
    })()
    return job.promise
  }

  attach(view: ClientView): HostAttachment {
    if (this.closed) throw new Error('Terminal backend disposed')
    const delegate = this.next.attach(view)
    let disposed = false
    const attachment = {
      deliver: async (frame: Record<string, unknown>) => {
        if (disposed) return
        try { if (!await view.postMessage(frame)) attachment.dispose() } catch { attachment.dispose() }
      },
      dispose: () => {
        if (disposed) return
        disposed = true
        this.attachments.delete(attachment)
        delegate.dispose()
      },
    }
    this.attachments.add(attachment)
    return {
      receive: async (frame: ClientFrame) => {
        if (disposed) throw new Error('Terminal view disposed')
        if (frame.kind === 'invoke' && INVOKES.has(frame.channel)) {
          try {
            if (frame.args.length !== (frame.channel === 'pty:spawn' ? 1 : 0)) throw new Error('Invalid terminal argument count')
            const result = frame.channel === 'shells:list' ? await discoverShells() : await this.spawn(frame.args[0])
            await attachment.deliver({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result })
          } catch (error) {
            await attachment.deliver({ kind: 'invoke-reply', id: frame.id, generation: frame.generation,
              ok: false, error: error instanceof Error ? error.message : 'Terminal operation failed' })
          }
          return
        }
        if (frame.kind === 'send' && SENDS.has(frame.channel)) {
          if (frame.args.length !== 1) throw new Error('Invalid terminal argument count')
          if (frame.channel === 'workspace:terminal-sync') {
            const tabId = identity(frame.args[0])
            const record = [...this.records.values()].find(r => r.tabId === tabId)
            const snapshot = async () => attachment.deliver({ kind: 'event', channel: 'workspace:terminal-state', args: [
              record ? { ptyId: record.ptyId, tabId, profile: record.profile, cols: record.cols, rows: record.rows,
                sequence: record.sequence, exitCode: record.exitCode, outputError: record.outputError, ...record.screen.snapshot() } : null, tabId,
            ] })
            if (record) await record.screen.order(snapshot)
            else await snapshot()
            return
          }
          const opts = object(frame.args[0])
          const record = typeof opts.ptyId === 'string' ? this.records.get(opts.ptyId) : undefined
          if (!record) throw new Error('Unknown workspace terminal')
          if (frame.channel === 'pty:kill') { this.remove(record); return }
          if (record.exitCode !== undefined || record.outputError) throw new Error('Terminal has exited')
          if (frame.channel === 'pty:write') {
            if (typeof opts.data !== 'string' || Buffer.byteLength(opts.data) > MAX_INPUT) throw new Error('Terminal input exceeds limit')
            this.sessions.writeToSession(record.ptyId, opts.data)
          } else {
            dimensions(opts.cols, opts.rows)
            const cols = opts.cols as number, rows = opts.rows as number
            const reserved = cells(cols, rows), delta = reserved - record.reserved
            if (this.reservedCells + delta > MAX_CELLS) throw new Error('Terminal screen resource limit reached')
            this.reservedCells += delta
            record.reserved = reserved
            await record.screen.order(() => {
              record.screen.resize(cols, rows)
              record.cols = cols; record.rows = rows
              this.sessions.resizeSession(record.ptyId, cols, rows)
            })
          }
          return
        }
        if (frame.kind === 'send') {
          if (frame.channel === 'disconnect') this.end()
          if (frame.channel === 'tab:close' || frame.channel === 'tab:disconnect') this.end(identity(frame.args[0]))
        }
        await delegate.receive(frame)
      },
      dispose: attachment.dispose,
    }
  }

  dispose(): void {
    if (this.closed) return
    this.closed = true
    this.end()
    for (const attachment of this.attachments) attachment.dispose()
  }
}
