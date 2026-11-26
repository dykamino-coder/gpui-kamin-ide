// In-process listing of the Glob tool for small trees: the same file set as
// `rg --files --hidden --no-ignore --no-config --follow --glob <pattern>
// --glob !<skipped>…` started in the base directory, without starting a
// process (~8 ms in a Linux container, ~50 ms on Windows for a tree read here
// in well under 1 ms). Returns null — run ripgrep — when the pattern, a
// symbolic link or the budget is outside what it replicates.
import fs from 'node:fs'
import path from 'node:path'
import { Overrides } from './rg-ignore'
import { defaultBudget, walk, type WalkBudget } from './walk'

export interface LocalGlobQuery {
  base: string
  pattern: string
  skipped: string[]
}

/** ripgrep matches anchored override globs against the path relative to its
 *  working directory as the OS reports it (resolved on Linux/macOS). */
function baseIsItsOwnRealPath(base: string): boolean {
  if (process.platform === 'win32') return true
  try {
    return fs.realpathSync(base) === base
  } catch {
    return false
  }
}

/** Matching files with ripgrep's path spelling (`/` separators), or null. */
export function localGlobFiles(q: LocalGlobQuery, budget: WalkBudget = defaultBudget()): string[] | null {
  if (!path.isAbsolute(q.base) || !baseIsItsOwnRealPath(q.base)) return null
  const overrides = Overrides.compile([q.pattern, ...q.skipped.map((d) => `!${d}`)])
  if (!overrides) return null
  const files: string[] = []
  const done = walk(q.base, budget, {
    dir: (_abs, rel) => overrides.verdict(rel, true) !== 'ignore',
    file: (abs, rel) => {
      if (overrides.verdict(rel, false) === 'whitelist') files.push(abs)
    },
  })
  if (!done) return null
  return process.platform === 'win32' ? files.map((f) => f.replace(/\\/g, '/')) : files
}
