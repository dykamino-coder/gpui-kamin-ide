import { describe, expect, it } from 'vitest'
import { terminalEnv } from './terminal-env'
import { defaultCodexHome } from '../codex/codex-home'

describe('admin terminal CODEX_HOME', () => {
  it('overrides inherited personal CODEX_HOME with the Bridge default without changing the shell HOME', () => {
    const base = { HOME: '/home/bridge', CODEX_HOME: '/personal/codex', CLAUDECODE: '1', CLAUDE_CODE_SESSION: 'nested' }
    const env = terminalEnv(base)
    expect(env.CODEX_HOME).toBe(defaultCodexHome(base))
    expect(env.CODEX_HOME).not.toBe(base.CODEX_HOME)
    expect(env.HOME).toBe('/home/bridge')
    expect(env).not.toHaveProperty('CLAUDECODE')
    expect(env).not.toHaveProperty('CLAUDE_CODE_SESSION')
    expect(base.CODEX_HOME).toBe('/personal/codex')
  })
  it('uses the container account after #848 and an explicit runtime home', () => {
    const base = { BRIDGE_CODEX_HOME: '/home/bridge/.codex', CODEX_HOME: '/wrong', BRIDGE_CONTAINER: '1' }
    expect(terminalEnv(base).CODEX_HOME).toBe('/home/bridge/.codex')
    expect(terminalEnv(base, '/bridge/runtime-home').CODEX_HOME).toBe('/bridge/runtime-home')
  })
})
