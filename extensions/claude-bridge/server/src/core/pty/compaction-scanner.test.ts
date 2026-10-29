import fsp from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { expect, it, vi } from 'vitest'
import { CompactionScanner } from './compaction-scanner'

it('does no metadata IO on idle ticks and detects a newly created compact file', async () => {
  const dir = await fsp.mkdtemp(join(tmpdir(), 'compact-dirty-'))
  const file = join(dir, 'current.jsonl')
  await fsp.writeFile(file, '{}\n')
  await fsp.utimes(file, new Date(1000), new Date(1000))
  const scanner = new CompactionScanner()
  const newer: string[] = []
  const unsub = scanner.subscribe({ dir, currentFilePath: file, sessionCreatedAt: 0, onNewer: (p) => newer.push(p) })
  try {
    await scanner.sweep()
    const stat = vi.spyOn(fsp, 'stat')
    const readdir = vi.spyOn(fsp, 'readdir')
    await scanner.sweep()
    await scanner.sweep()
    expect(stat).not.toHaveBeenCalled()
    expect(readdir).not.toHaveBeenCalled()
    const compact = join(dir, 'compact.jsonl')
    await fsp.writeFile(compact, '{}\n')
    await expect
      .poll(async () => {
        await scanner.sweep()
        return newer.includes(compact)
      })
      .toBe(true)
    expect(readdir).not.toHaveBeenCalled()
  } finally {
    unsub()
    vi.restoreAllMocks()
    await fsp.rm(dir, { recursive: true, force: true })
  }
})
