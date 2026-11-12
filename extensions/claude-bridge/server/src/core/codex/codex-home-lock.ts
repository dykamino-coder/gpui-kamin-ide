// ============================================================================
// One app-server per CODEX_HOME — across processes, not just inside one.
//
// Field lesson (Codex agent driver, 2026-10/11): a second app-server on the
// same thread store ("thread-store conflict: already has an active writer")
// broke the thread and hung the next resume. Inside one Bridge process the
// CodexHostRegistry already keeps a single app-server per home; this lock also
// stops a second Bridge process on the same host from starting its own.
//
// The lock names its owner: host name, boot id, pid and the process start
// time. Only an owner proven alive keeps it: same host, same boot, the pid
// running, and that pid still the process that wrote the lock (not a reused
// pid). Anything else — another host (a replaced container: it has a new
// host name and its processes are gone), a reboot, a dead or reused pid — is
// taken over at once. Field lesson (acceptance 2026-11-12): the old rule
// waited 90 s for a silent heartbeat, so every server update left ChatGPT
// unavailable for that long ("another Bridge server already runs codex").
// ============================================================================

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

export const CODEX_HOME_LOCK_FILE = 'bridge-app-server.lock'

export interface CodexHomeLockOwner {
  pid?: number
  host?: string
  /** Kernel boot id (Linux); a different one means the owner did not survive a reboot. */
  bootId?: string | null
  /** Process start time of `pid` (Linux /proc ticks); tells a reused pid apart. */
  procStart?: string | null
  startedAt?: string
}

export class CodexHomeBusyError extends Error {
  constructor(readonly owner: CodexHomeLockOwner) {
    super(
      `another Bridge server already runs codex on this CODEX_HOME (pid ${owner.pid ?? '?'} on ${owner.host ?? 'unknown host'})`,
    )
    this.name = 'CodexHomeBusyError'
  }
}

export interface CodexHomeLock {
  release(): void
}

/** How the lock sees this machine; tests replace each probe. */
export interface LockOptions {
  now?: () => number
  pid?: number
  host?: string
  bootId?: () => string | null
  /** Start time of a running process, null when it does not run or is unknown. */
  procStart?: (pid: number) => string | null
  isAlive?: (pid: number) => boolean
}

function pidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === 'EPERM'
  }
}

function readBootId(): string | null {
  try {
    return fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim() || null
  } catch {
    return null
  }
}

/** Field 22 of /proc/<pid>/stat (after the parenthesized command name). */
function readProcStart(pid: number): string | null {
  try {
    const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8')
    return stat.slice(stat.lastIndexOf(')') + 2).split(' ')[19] ?? null
  } catch {
    return null
  }
}

/** Whether the lock's owner is proven to be a live process (then it wins). */
export function ownerAlive(
  owner: CodexHomeLockOwner,
  me: Required<Pick<LockOptions, 'host' | 'isAlive'>> & {
    bootId: string | null
    procStart: (pid: number) => string | null
  },
): boolean {
  if (owner.host !== me.host || typeof owner.pid !== 'number') return false
  if (owner.bootId && me.bootId && owner.bootId !== me.bootId) return false
  if (!me.isAlive(owner.pid)) return false
  // A recorded start time must match the running process; without one (a lock
  // from an older Bridge, or no /proc) a live pid on this host is trusted.
  if (owner.procStart) {
    const running = me.procStart(owner.pid)
    if (running && running !== owner.procStart) return false
  }
  return true
}

export function acquireCodexHomeLock(home: string, options: LockOptions = {}): CodexHomeLock {
  const file = path.join(home, CODEX_HOME_LOCK_FILE)
  const pid = options.pid ?? process.pid
  const host = options.host ?? os.hostname()
  const now = options.now ?? Date.now
  const procStart = options.procStart ?? readProcStart
  const me = {
    host,
    isAlive: options.isAlive ?? pidAlive,
    bootId: (options.bootId ?? readBootId)(),
    procStart,
  }
  fs.mkdirSync(home, { recursive: true })
  const owner: CodexHomeLockOwner = {
    pid,
    host,
    bootId: me.bootId,
    procStart: procStart(pid),
    startedAt: new Date(now()).toISOString(),
  }
  const body = JSON.stringify(owner)

  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      fs.writeFileSync(file, body, { flag: 'wx', mode: 0o600 })
      return heldLock(file, pid, host)
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error
    }
    let current: CodexHomeLockOwner = {}
    try {
      current = JSON.parse(fs.readFileSync(file, 'utf8')) as CodexHomeLockOwner
    } catch {
      // Unreadable / half-written: nobody can prove ownership.
    }
    const ours = current.host === host && current.pid === pid
    if (!ours && ownerAlive(current, me)) throw new CodexHomeBusyError(current)
    fs.rmSync(file, { force: true })
  }
  throw new CodexHomeBusyError({})
}

function heldLock(file: string, pid: number, host: string): CodexHomeLock {
  let released = false
  return {
    release() {
      if (released) return
      released = true
      try {
        const owner = JSON.parse(fs.readFileSync(file, 'utf8')) as CodexHomeLockOwner
        if (owner.pid === pid && owner.host === host) fs.rmSync(file, { force: true })
      } catch {
        // Already gone.
      }
    },
  }
}
