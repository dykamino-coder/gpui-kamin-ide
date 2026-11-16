// Bounded in-process directory walk: depth first, pre-order, entries in the
// operating system's directory order (ripgrep does not sort without `--sort`;
// its parallel walk prints files in an order of its own). For small trees only:
// it gives up (returns null) past an entry, byte or time budget, or on an
// entry whose handling it does not replicate (a symbolic link, an unreadable
// directory), and the caller then runs ripgrep. Synchronous I/O: within the
// budget it costs less than one ripgrep process start blocks the loop.
import fs from 'node:fs'
import path from 'node:path'

/** Limits of one in-process search. */
export interface WalkBudget {
  entries: number
  bytes: number
  /** performance.now() deadline. */
  deadline: number
}

/** The walk competes with one ripgrep run: ~6–8 ms in a Linux container and
 *  ~45–50 ms on Windows for a small tree, where the in-process search of
 *  25 files takes ~1.3 ms and ~5 ms. It gets about half of that before it
 *  gives up, so a tree too large for it costs at most that much extra. */
export function defaultBudget(now = performance.now()): WalkBudget {
  return { entries: 4000, bytes: 4 * 1024 * 1024, deadline: now + (process.platform === 'win32' ? 25 : 6) }
}

export class WalkAbort extends Error {}

/** The tree is larger than the budget. */
export class WalkBudgetExceeded extends WalkAbort {}

// A routing hint, not a cache of results: a root whose walk ran out of budget
// (and any search covering it) goes straight to ripgrep for a while, so a
// large repository does not pay a failed in-process attempt on every call.
// Output is the same either way; a stale hint only costs speed.
const TOO_BIG_TTL_MS = 5 * 60_000
const TOO_BIG_MAX = 64
const tooBig = new Map<string, number>()

function coveredByTooBig(root: string, now: number): boolean {
  for (const [p, until] of tooBig) {
    if (until <= now) tooBig.delete(p)
    else if (p === root || p.startsWith(root.endsWith(path.sep) ? root : root + path.sep)) return true
  }
  return false
}

function rememberTooBig(root: string, now: number): void {
  tooBig.delete(root)
  tooBig.set(root, now + TOO_BIG_TTL_MS)
  if (tooBig.size > TOO_BIG_MAX) tooBig.delete(tooBig.keys().next().value!)
}

/** Test hook: forget the roots that ran out of budget. */
export function resetWalkRoutingForTests(): void {
  tooBig.clear()
}

export interface WalkVisitor {
  /** Whether to descend into `abs` (rel: `/`-separated, relative to the root). */
  dir(abs: string, rel: string): boolean
  /** Entering a directory (the root too) with its entry names; may throw WalkAbort. */
  enter?(abs: string, rel: string, names: string[]): void
  /** Leaving a directory entered before. */
  leave?(abs: string): void
  /** A regular file. */
  file(abs: string, rel: string): void
}

/** Joins as ripgrep prints a walked path: no separator doubled after a root. */
export function joinShown(base: string, name: string, sep = path.sep): string {
  return base.endsWith(sep) || base.endsWith('/') ? base + name : base + sep + name
}

/** Walks `root`; false when the budget or an unreplicated entry stopped it,
 *  or when a walk of it or of a directory under it ran out of budget lately. */
export function walk(root: string, budget: WalkBudget, visitor: WalkVisitor): boolean {
  let entries = 0
  const visit = (dir: string, relDir: string): void => {
    let list: fs.Dirent[]
    try {
      list = fs.readdirSync(dir, { withFileTypes: true })
    } catch {
      throw new WalkAbort()
    }
    entries += list.length
    if (entries > budget.entries || performance.now() > budget.deadline) throw new WalkBudgetExceeded()
    visitor.enter?.(dir, relDir, list.map((d) => d.name))
    for (const d of list) {
      const abs = joinShown(dir, d.name)
      const rel = relDir ? `${relDir}/${d.name}` : d.name
      if (d.isSymbolicLink()) throw new WalkAbort()
      if (d.isDirectory()) {
        if (visitor.dir(abs, rel)) visit(abs, rel)
      } else if (d.isFile()) visitor.file(abs, rel)
    }
    visitor.leave?.(dir)
  }
  if (coveredByTooBig(root, performance.now())) return false
  try {
    visit(root, '')
    return true
  } catch (err) {
    if (err instanceof WalkBudgetExceeded) rememberTooBig(root, performance.now())
    if (err instanceof WalkAbort) return false
    throw err
  }
}
