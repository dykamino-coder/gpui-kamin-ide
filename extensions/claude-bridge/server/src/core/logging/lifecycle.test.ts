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
it('INC-2026-0070: records conversation identity only as stable refs', async () => {
  const conversation = '00000000-0000-4000-8000-000000000001'
  const next = '00000000-0000-4000-8000-000000000002'
  lifecycleLog('session_request', {
    requestKind: 'resume',
    resumeRequested: true,
    resumeConversationId: conversation,
  })
  lifecycleLog('session_create', {
    sessionId: 'fake-session',
    resumeRequested: true,
    resumeConversationId: conversation,
    resumeNotFound: true,
    settingsDirFound: false,
    settingsDirSource: 'fresh_not_found',
  })
  lifecycleLog('conversation_id', {
    sessionId: 'fake-session',
    conversationId: next,
    previousConversationId: conversation,
    idSource: 'changed',
  })
  lifecycleLog('session_request', { requestKind: 'private-kind', settingsDirSource: '/private/dir', idSource: 'x' })
  const content = fs.readFileSync(path.join(directory, 'lifecycle.jsonl'), 'utf8')
  const rows = content
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line))
  // Fixed vector shared with the client's incident log (same domain string).
  const { conversationRef } = await import('./lifecycle')
  expect(conversationRef(conversation)).toBe('53d31033b678')
  expect(conversationRef(next)).toBe('a09e04d7ed68')
  expect(rows[0]).toMatchObject({ event: 'session_request', requestKind: 'resume', resumeRef: '53d31033b678' })
  expect(rows[1]).toMatchObject({
    event: 'session_create',
    resumeRequested: true,
    resumeRef: '53d31033b678',
    resumeNotFound: true,
    settingsDirFound: false,
    settingsDirSource: 'fresh_not_found',
  })
  expect(rows[2]).toMatchObject({
    event: 'conversation_id',
    conversationRef: 'a09e04d7ed68',
    previousRef: '53d31033b678',
    idSource: 'changed',
  })
  expect(rows[3]).toEqual({
    v: 1,
    timestamp: rows[3].timestamp,
    boot: rows[3].boot,
    sequence: rows[3].sequence,
    event: 'session_request',
  })
  expect(content).not.toContain(conversation)
  expect(content).not.toContain(next)
  expect(content).not.toMatch(/private|fake-session/)
})
