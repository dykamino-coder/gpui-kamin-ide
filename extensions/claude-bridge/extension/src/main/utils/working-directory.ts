// One rule for the directory client-side work runs in: hooks, shell tools,
// file tools with relative paths, plugin monitors. A tab may have no folder,
// and a remembered folder may be gone; a process started in a missing
// directory fails with ENOENT that names the program, and the host process's
// own directory (the application install) is never what the user means.

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

function isDirectory(candidate: string): boolean {
  try { return fs.statSync(candidate).isDirectory() } catch { return false }
}

/** First candidate that is an existing directory on this host, otherwise the
 * user's home directory. Candidates are tried in the given order. */
export function resolveWorkingDirectory(...candidates: Array<string | null | undefined>): string {
  for (const candidate of candidates) {
    if (typeof candidate === 'string' && candidate.trim() && isDirectory(candidate)) return candidate
  }
  return os.homedir()
}

/** Absolute path for a tool argument. An absolute path is returned unchanged;
 * a relative one is resolved against the working directory chosen above, not
 * against the directory of the host process. */
export function resolveToolPath(target: string, workingDirectory?: string | null): string {
  if (path.isAbsolute(target)) return target
  return path.resolve(resolveWorkingDirectory(workingDirectory), target)
}
