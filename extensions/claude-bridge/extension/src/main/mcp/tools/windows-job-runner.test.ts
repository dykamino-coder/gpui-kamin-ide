import { join } from 'node:path'
import { describe, expect, it, vi } from 'vitest'
import { findWindowsJobRunner, type RunnerProbe } from './windows-job-runner'

const hostDir = join('install', 'runtime')
const shipped = join(hostDir, '..', 'kamin-jobrun.exe')
const probe = (over: Partial<RunnerProbe>): RunnerProbe => ({
  platform: 'win32',
  env: {},
  execPath: join(hostDir, 'kaminhost.exe'),
  exists: (path) => path === shipped,
  works: () => true,
  ...over,
})

describe('Windows job runner lookup', () => {
  it('uses the runner shipped beside the runtime directory', () => {
    expect(findWindowsJobRunner(probe({}))).toBe(shipped)
  })

  it('is absent on other platforms without touching the disk', () => {
    const exists = vi.fn(() => true)
    expect(findWindowsJobRunner(probe({ platform: 'linux', exists }))).toBeNull()
    expect(exists).not.toHaveBeenCalled()
  })

  it('prefers an explicit path and falls back to the host directory', () => {
    const explicit = join('elsewhere', 'kamin-jobrun.exe')
    const env = { KAMIN_JOBRUN_EXE: explicit }
    expect(findWindowsJobRunner(probe({ env, exists: () => true }))).toBe(explicit)
    const beside = join(hostDir, 'kamin-jobrun.exe')
    expect(findWindowsJobRunner(probe({ exists: (path) => path === beside }))).toBe(beside)
  })

  it('is absent when no runner exists or the found one cannot set a job up', () => {
    expect(findWindowsJobRunner(probe({ exists: () => false }))).toBeNull()
    expect(findWindowsJobRunner(probe({ works: () => false }))).toBeNull()
  })
})