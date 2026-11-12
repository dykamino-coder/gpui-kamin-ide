// ============================================================================
// One app-server per CODEX_HOME — across processes, not just inside one.
//
// Field lesson (Codex agent driver, 2026-10/11): a second app-server on the
// same thread store ("thread-store conflict: already has an active writer")
// broke the thread and hung the next resume. Inside one Bridge process the
// CodexHostRegistry already keeps a single app-server per home; this lock also
// stops a second Bridge (an overlapping container, a dev server on the same
// volume) from starting its own. The owner refreshes the file's mtime; a lock
// whose heartbeat stopped (crashed owner, other host) is taken over.
// ============================================================================

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

export const CODEX_HOME_LOCK_FILE = 'bridge-app-server.lock'
const HEARTBEAT_MS = 30_000
const STALE_MS = 3 * HEARTBEAT_MS

export class CodexHomeBusyError extends Error {
  constructor(readonly owner: { pid?: number; host?: string }) {
    super(
      `another Bridge server already runs codex on this CODEX_HOME (pid ${owner.pid ?? '?'} on ${owner.host ?? 'unknown host'})`,
    )
    this.name = 'CodexHomeBusyError'
  }
}

export interface CodexHomeLock {
  release(): void
}

interface LockOptions {
  now?: () => number
  pid?: number
  host?: string
  isAlive?: (pid: number) => boolean
  heartbeatMs?: number
  staleMs?: number
}

function pidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === 'EPERM'
  }
}

export function acquireCodexHomeLock(home: string, options: LockOptions = {}): CodexHomeLock {
  const file = path.join(home, CODEX_HOME_LOCK_FILE)
  const pid = options.pid ?? process.pid
  const host = options.host ?? os.hostname()
  const now = options.now ?? Date.now
  const staleMs = options.staleMs ?? STALE_MS
  const isAlive = options.isAlive ?? pidAlive
  fs.mkdirSync(home, { recursive: true })
  const body = JSON.stringify({ pid, host, startedAt: new Date(now()).toISOString() })

  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      fs.writeFileSync(file, body, { flag: 'wx', mode: 0o600 })
      return heldLock(file, pid, host, options.heartbeatMs ?? HEARTBEAT_MS)
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error
    }
    let owner: { pid?: number; host?: string } = {}
    let mtime = 0
    try {
      owner = JSON.parse(fs.readFileSync(file, 'utf8')) as typeof owner
      mtime = fs.statSync(file).mtimeMs
    } catch {
      // Unreadable / half-written: treat as stale.
    }
    const sameHost = owner.host === host
    const ours = sameHost && owner.pid === pid
    const deadHere = sameHost && typeof owner.pid === 'number' && !isAlive(owner.pid)
    const silent = now() - mtime > staleMs
    if (!ours && !deadHere && !silent) throw new CodexHomeBusyError(owner)
    fs.rmSync(file, { force: true })
  }
  throw new CodexHomeBusyError({})
}

function heldLock(file: string, pid: number, host: string, heartbeatMs: number): CodexHomeLock {
  const beat = setInterval(() => {
    try {
      const t = new Date()
      fs.utimesSync(file, t, t)
    } catch {
      // A removed lock is re-taken by the next start; nothing to do here.
    }
  }, heartbeatMs)
  beat.unref?.()
  let released = false
  return {
    release() {
      if (released) return
      released = true
      clearInterval(beat)
      try {
        const owner = JSON.parse(fs.readFileSync(file, 'utf8')) as { pid?: number; host?: string }
        if (owner.pid === pid && owner.host === host) fs.rmSync(file, { force: true })
      } catch {
        // Already gone.
      }
    },
  }
}
