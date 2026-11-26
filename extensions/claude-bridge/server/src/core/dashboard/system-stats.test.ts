import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import { VirtualPty } from '../codex/virtual-pty'

const proc = vi.hoisted(() => ({ readdir: vi.fn(), readFile: vi.fn() }))
vi.mock('fs/promises', () => proc)
import { getProcessTreeStats, startSystemStatsSampler } from './system-stats'

function stat(pid: number, ppid: number, rss: number): string {
  const fields = Array<string>(22).fill('0')
  fields[0] = 'S'
  fields[1] = String(ppid)
  fields[11] = '100'
  fields[12] = '20'
  fields[21] = String(rss)
  return `${pid} (synthetic process) ${fields.join(' ')}`
}

beforeAll(async () => {
  vi.useFakeTimers()
  const rows = new Map([
    ['/proc/1/stat', stat(1, 0, 256)],
    ['/proc/10/stat', stat(10, 1, 64)],
    ['/proc/11/stat', stat(11, 10, 16)],
    ['/proc/20/stat', stat(20, 1, 128)],
    // Parent vanished between samples: its former child is not a live root.
    ['/proc/31/stat', stat(31, 30, 8)],
  ])
  proc.readdir.mockResolvedValue(['1', '10', '11', '20', '31'])
  proc.readFile.mockImplementation(async (file: string) => rows.get(file))
  startSystemStatsSampler()
  await vi.waitFor(() => expect(getProcessTreeStats(10)).not.toBeNull())
})

afterAll(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
})

describe('per-session process accounting', () => {
  it('does not charge the container tree to a Codex virtual PTY with pid 0', () => {
    const pty = new VirtualPty({ interrupt: () => {}, stop: async () => 0 })
    expect(pty.pid).toBe(0)
    expect(getProcessTreeStats(pty.pid)).toBeNull()
  })

  it('includes a real PTY and its descendants, excluding sibling sessions', () => {
    expect(getProcessTreeStats(10)).toEqual({ cpuPercent: 0, memBytes: (64 + 16) * 4096 })
    expect(getProcessTreeStats(20)).toEqual({ cpuPercent: 0, memBytes: 128 * 4096 })
  })

  it('does not attribute orphaned descendants to an absent root', () => {
    expect(getProcessTreeStats(30)).toBeNull()
  })

  it.each([undefined, -1, Number.NaN, Number.POSITIVE_INFINITY, 999])('ignores invalid or absent PID %s', (pid) => {
    expect(getProcessTreeStats(pid)).toBeNull()
  })
})
