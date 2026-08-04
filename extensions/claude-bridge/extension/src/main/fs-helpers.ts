import fs from 'fs'

export function silentRm(target: string): void {
  try { fs.rmSync(target, { recursive: true, force: true }) } catch { /* best-effort */ }
}
