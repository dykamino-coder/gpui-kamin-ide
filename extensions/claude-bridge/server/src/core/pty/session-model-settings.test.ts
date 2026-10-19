// Bridge owns launch models through CLI arguments, never through a stale
// project-local pin that /model would warn about after a live switch.
import 'reflect-metadata'
import fs from 'node:fs'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const roots = vi.hoisted(() => ({ home: '', sync: '' }))
vi.mock('os', async (original) => {
  const actual = await original<typeof import('os')>()
  const mocked = { ...actual, homedir: () => roots.home }
  return { ...mocked, default: mocked }
})
vi.mock('../sync/routes', () => ({
  getUserSyncDir: () => roots.sync,
  getProjectSyncDir: () => roots.sync,
}))

import { writeSessionSettings } from './session-settings'
import { buildClaudeArgs } from './session-env'

let root = ''
let settingsDir = ''
const localFile = () => path.join(settingsDir, '.claude', 'settings.local.json')

beforeEach(() => {
  root = fs.mkdtempSync(path.join(process.env.TMPDIR || '/tmp', 'bridge-model-settings-'))
  roots.home = path.join(root, 'home')
  roots.sync = path.join(root, 'sync')
  settingsDir = path.join(root, 'session')
  fs.mkdirSync(roots.home, { recursive: true })
  fs.mkdirSync(settingsDir, { recursive: true })
})
afterEach(() => fs.rmSync(root, { recursive: true, force: true }))

describe('Bridge launch model settings', () => {
  it.each([undefined, 'claude-opus-5', 'claude-opus-5-5'])('launches %s without a second model pin', (model) => {
    writeSessionSettings('synthetic-session', settingsDir, 'synthetic-mcp-token')
    const local = JSON.parse(fs.readFileSync(localFile(), 'utf8'))
    expect(local).not.toHaveProperty('model')
    expect(local.skipDangerousModePermissionPrompt).toBe(true)
    expect(local.env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS).toBe('1')
    const args = buildClaudeArgs({ model })
    expect(args[args.indexOf('--model') + 1]).toBe(model ?? 'claude-opus-5-5')
  })

  it('removes the generated pin from a previous Bridge version on resume', () => {
    fs.mkdirSync(path.dirname(localFile()), { recursive: true })
    fs.writeFileSync(localFile(), JSON.stringify({ model: 'claude-opus-5' }))
    writeSessionSettings('synthetic-session', settingsDir, 'synthetic-mcp-token')
    expect(JSON.parse(fs.readFileSync(localFile(), 'utf8'))).not.toHaveProperty('model')
    const args = buildClaudeArgs({ resumeConversationId: 'synthetic-conversation', model: 'claude-opus-5-5' })
    expect(args[args.indexOf('--model') + 1]).toBe('claude-opus-5-5')
  })

  it('leaves native transcript restore unpinned when Bridge has no model evidence', () => {
    writeSessionSettings('synthetic-session', settingsDir, 'synthetic-mcp-token')
    const args = buildClaudeArgs({ resumeConversationId: 'synthetic-conversation' })
    expect(args).not.toContain('--model')
    expect(JSON.parse(fs.readFileSync(localFile(), 'utf8'))).not.toHaveProperty('model')
  })

  it('does not use or modify the shared CLI user default for a fresh Bridge session', () => {
    const userFile = path.join(roots.home, '.claude', 'settings.json')
    fs.mkdirSync(path.dirname(userFile), { recursive: true })
    const shared = JSON.stringify({ model: 'claude-haiku-5-5' })
    fs.writeFileSync(userFile, shared)
    writeSessionSettings('synthetic-session', settingsDir, 'synthetic-mcp-token')
    const args = buildClaudeArgs({})
    expect(args[args.indexOf('--model') + 1]).toBe('claude-opus-5-5')
    expect(fs.readFileSync(userFile, 'utf8')).toBe(shared)
  })
})
