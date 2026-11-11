import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { clearSession, listSession, registerSessionHooks } from './registry'
import { mergeAllHooks } from './source-merger'
import type { HookSettings } from './types'

const SESSION = 'registry-client-source'
afterEach(() => clearSession(SESSION))

// Events the resolver prefers on the server, plus a tool that runs inside the CLI.
const hooks = (): HookSettings => ({
  UserPromptSubmit: [{ hooks: [{ type: 'command', command: 'node "C:\\Users\\me\\hook.js"' }] }],
  SubagentStop: [{ hooks: [{ type: 'command', command: 'node "C:\\Users\\me\\hook.js"' }] }],
  PreToolUse: [{ matcher: 'Agent', hooks: [{ type: 'command', command: 'node "C:\\Users\\me\\hook.js"' }] }],
})

function hosts(): string[] {
  return listSession(SESSION)
    .map((hook) => `${hook.event}:${hook.effectiveHost}`)
    .sort()
}

describe('hook host for client settings', () => {
  it('runs command hooks from the client settings on the client for every event', () => {
    registerSessionHooks(SESSION, hooks(), { kind: 'user', client: true })
    expect(hosts()).toEqual(['PreToolUse:local', 'SubagentStop:local', 'UserPromptSubmit:local'])
    clearSession(SESSION)

    registerSessionHooks(SESSION, hooks(), { kind: 'project', projectPath: 'C:\\proj', client: true })
    expect(hosts()).toEqual(['PreToolUse:local', 'SubagentStop:local', 'UserPromptSubmit:local'])
  })

  it('keeps an explicit host and the resolver defaults of server-side settings', () => {
    const explicit: HookSettings = {
      UserPromptSubmit: [{ hooks: [{ type: 'command', command: 'jq .', host: 'server' }] }],
    }
    registerSessionHooks(SESSION, explicit, { kind: 'user', client: true })
    expect(hosts()).toEqual(['UserPromptSubmit:server'])
    clearSession(SESSION)

    registerSessionHooks(SESSION, hooks(), { kind: 'user' })
    expect(hosts()).toEqual(['PreToolUse:server', 'SubagentStop:server', 'UserPromptSubmit:server'])
  })

  it('marks hooks merged from synced settings files as client hooks', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-synced-hooks-'))
    try {
      const userFile = path.join(dir, 'user.json')
      const projectFile = path.join(dir, 'project.json')
      fs.writeFileSync(userFile, JSON.stringify({ hooks: { UserPromptSubmit: hooks().UserPromptSubmit } }))
      fs.writeFileSync(projectFile, JSON.stringify({ hooks: { SubagentStop: hooks().SubagentStop } }))
      const merged = mergeAllHooks('C:\\proj', [userFile, projectFile])
      const sources = [...merged.sourceByMatcher.values()].filter((source) => 'client' in source)
      expect(sources).toEqual([
        { kind: 'user', client: true },
        { kind: 'project', projectPath: 'C:\\proj', client: true },
      ])
    } finally {
      fs.rmSync(dir, { recursive: true, force: true })
    }
  })
})
