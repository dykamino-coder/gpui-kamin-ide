// Filesystem bounds/privacy and failure isolation on the production journal.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { lifecycleLog, LIFECYCLE_BACKUPS, LIFECYCLE_FILE_BYTES } from './lifecycle'

let directory: string
beforeEach(() => {
  directory = fs.mkdtempSync(path.join(process.env.BRIDGE_TEST_TMP || os.tmpdir(), 'lifecycle-files-'))
  vi.stubEnv('BRIDGE_LIFECYCLE_LOG_DIR', directory)
})
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllEnvs()
})

it('allowlists keys and enum values and pseudonymizes even hostile session IDs', () => {
  const fields = {
    sessionId: 'Bearer fake-secret\n../private-project',
    reason: 'explicit_end',
    ageMs: 5,
    idleMs: Infinity,
    exitCode: 129,
    isSubAgent: true,
    token: 'fake-secret',
    prompt: 'private-prompt',
    error: 'private-error',
  }
  lifecycleLog('session_destroy', fields)
  lifecycleLog('session_destroy', { ...fields, reason: 'private-reason' })
  lifecycleLog('private-event', fields)
  const content = fs.readFileSync(path.join(directory, 'lifecycle.jsonl'), 'utf8')
  const rows = content
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line))
  expect(rows).toHaveLength(2)
  expect(rows[0]).toMatchObject({ reason: 'explicit_end', ageMs: 5, exitCode: 129, isSubAgent: true })
  expect(rows[0].session).toMatch(/^[a-f0-9]{24}$/)
  expect(rows[1].session).toBe(rows[0].session)
  expect(rows[1]).not.toHaveProperty('reason')
  expect(rows[0]).not.toHaveProperty('idleMs')
  expect(content).not.toMatch(/private-|fake-secret|Bearer/)
})
it('rotates complete records and bounds retained disk across fresh logger imports', async () => {
  // Pre-fill each generation to force real rename/retirement, not a policy mock.
  const file = path.join(directory, 'lifecycle.jsonl')
  for (let n = 0; n <= LIFECYCLE_BACKUPS; n++) {
    fs.writeFileSync(n ? `${file}.${n}` : file, 'x'.repeat(LIFECYCLE_FILE_BYTES))
  }
  lifecycleLog('server_start')
  expect(fs.readdirSync(directory)).toHaveLength(LIFECYCLE_BACKUPS + 1)
  expect(JSON.parse(fs.readFileSync(file, 'utf8')).event).toBe('server_start')
  expect(
    fs.readdirSync(directory).reduce((sum, name) => sum + fs.statSync(path.join(directory, name)).size, 0),
  ).toBeLessThanOrEqual((LIFECYCLE_BACKUPS + 1) * LIFECYCLE_FILE_BYTES)
  vi.resetModules()
  const fresh = await import('./lifecycle')
  fresh.lifecycleLog('server_start')
  expect(fs.readFileSync(file, 'utf8').trim().split('\n')).toHaveLength(2)
})
it('does not change application outcomes or print private paths when storage fails', () => {
  const blocker = path.join(directory, 'blocker')
  fs.writeFileSync(blocker, '')
  vi.stubEnv('BRIDGE_LIFECYCLE_LOG_DIR', blocker)
  const warning = vi.spyOn(console, 'warn').mockImplementation(() => {})
  expect(() => lifecycleLog('server_start')).not.toThrow()
  lifecycleLog('server_start')
  expect(warning).toHaveBeenCalledTimes(1)
  expect(warning.mock.calls.flat().join(' ')).not.toContain(directory)
})
