import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

let stateRoot: string | undefined
const migratedFiles = ['tabs-state.json', 'plugin-hook-approvals.json', 'mcp-servers.json', 'mcp-tool-schemas.json']
const migrationMarker = '.home-state-migrated-v1'

/** Must run before constructing any Bridge store. Home is a read-only upgrade
 * source for the default profile, never the live persistence directory. */
export function initBridgeState(storageDir: string, options: { isDefaultProfile?: boolean; legacyDir?: string } = {}): void {
  stateRoot = storageDir
  if (!options.isDefaultProfile) return
  const marker = path.join(storageDir, migrationMarker)
  try {
    if (fs.existsSync(marker)) return
    const legacyDir = options.legacyDir ?? path.join(os.homedir(), '.claude', 'open-claude-bridge')
    fs.mkdirSync(storageDir, { recursive: true })
    for (const name of migratedFiles) {
      const source = path.join(legacyDir, name)
      const target = path.join(storageDir, name)
      if (fs.existsSync(target) || !fs.existsSync(source)) continue
      // Existing profile state wins, including if another window wrote it while
      // this window was starting. Never modify or remove the legacy source.
      try { fs.copyFileSync(source, target, fs.constants.COPYFILE_EXCL) }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error }
    }
    fs.writeFileSync(marker, '1\n', 'utf8')
  } catch (error) {
    // Keep the profile usable; without the marker the next startup can retry.
    console.warn('[bridge] Home-state migration deferred:', error)
  }
}

export function bridgeStatePath(...parts: string[]): string {
  if (!stateRoot) throw new Error('Bridge state directory has not been initialized by the host')
  return path.join(stateRoot, ...parts)
}
