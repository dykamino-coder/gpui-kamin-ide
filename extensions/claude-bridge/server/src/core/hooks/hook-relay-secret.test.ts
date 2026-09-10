import { describe, expect, it } from 'vitest'

import { buildHookCommandRelay, HOOK_RELAY_TOKEN_ENV, rewriteHooksForCli } from './proxy-rewriter'
import { clearSession } from './registry'
import type { HookSettings } from './types'

const SESSION_TOKEN = '6f0c4a52-7f0a-4d02-9c3f-0e6a4b0d1a77'

describe('hook relay credential never reaches a shown surface', () => {
  it('builds a relay command that reads the credential from the environment', () => {
    const script = buildHookCommandRelay('http://127.0.0.1:3456/api/hooks/s1/SessionEnd/h1')

    expect(script).toContain(HOOK_RELAY_TOKEN_ENV)
    expect(script).toContain('"Bearer "+k')
    expect(script).not.toContain(SESSION_TOKEN)
    expect(script).not.toMatch(/Bearer [0-9a-f-]{8,}/)
  })

  it('leaves no token in the rewritten declaration that Console, JSONL and settings echo', () => {
    const hooks: HookSettings = {
      SessionEnd: [{ hooks: [{ type: 'command', command: 'notify.sh' }] }],
      PreToolUse: [{ matcher: 'Bash', hooks: [{ type: 'command', command: 'guard.sh' }] }],
    }
    try {
      const rewritten = rewriteHooksForCli('secret-free-session', hooks, { kind: 'user' })
      const declaration = JSON.stringify(rewritten)

      expect(declaration).toContain('/api/hooks/secret-free-session/')
      expect(declaration).not.toContain(SESSION_TOKEN)
      // The only `Bearer` left is the runtime concatenation with the env value;
      // a literal credential after it is what must never appear.
      expect(declaration).toContain(HOOK_RELAY_TOKEN_ENV)
      expect(declaration).not.toMatch(/Bearer [0-9a-zA-Z-]/)
    } finally {
      clearSession('secret-free-session')
    }
  })

  it('fails closed when the environment carries no credential', async () => {
    const script = buildHookCommandRelay('http://127.0.0.1:1/api/hooks/s1/SessionEnd/h1')
    const { spawn } = await import('node:child_process')
    const env = { ...process.env }
    delete env[HOOK_RELAY_TOKEN_ENV]

    const result = await new Promise<{ stderr: string; exitCode: number | null }>((resolve, reject) => {
      const child = spawn(process.execPath, ['-e', script], { stdio: ['pipe', 'pipe', 'pipe'], env })
      let stderr = ''
      child.stderr.on('data', chunk => { stderr += chunk.toString() })
      child.on('error', reject)
      child.on('close', exitCode => resolve({ stderr, exitCode }))
      child.stdin.end('{"hook_event_name":"SessionEnd"}')
    })

    expect(result.stderr).toBe('Missing hook relay credential')
    expect(result.exitCode).toBe(1)
  })
})
