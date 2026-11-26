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

describe('admin terminal codex network env', () => {
  const dashboardCa = (env: Record<string, string>) => {
    env.NODE_EXTRA_CA_CERTS = '/data/certs/corp.pem'
  }
  const none = () => {}

  it('passes the dashboard CA to codex as CODEX_CA_CERTIFICATE', () => {
    const env = terminalEnv({ HOME: '/home/bridge' }, '/codex', dashboardCa)
    expect(env.NODE_EXTRA_CA_CERTS).toBe('/data/certs/corp.pem')
    expect(env.CODEX_CA_CERTIFICATE).toBe('/data/certs/corp.pem')
  })
  it('keeps an explicit CODEX_CA_CERTIFICATE', () => {
    const env = terminalEnv({ CODEX_CA_CERTIFICATE: '/own/ca.pem' }, '/codex', dashboardCa)
    expect(env.CODEX_CA_CERTIFICATE).toBe('/own/ca.pem')
  })
  it('adds no CODEX_CA_CERTIFICATE without a CA', () => {
    expect(terminalEnv({ HOME: '/home/bridge' }, '/codex', none)).not.toHaveProperty('CODEX_CA_CERTIFICATE')
  })
  it('gives inherited proxies the loopback bypass', () => {
    const env = terminalEnv({ ALL_PROXY: 'http://proxy:3128', NO_PROXY: 'corp.internal' }, '/codex', none)
    expect(env.NO_PROXY).toBe('corp.internal,127.0.0.1,localhost,::1')
    expect(env.no_proxy).toBe(env.NO_PROXY)
    expect(terminalEnv({ HOME: '/h' }, '/codex', none)).not.toHaveProperty('NO_PROXY')
  })
})
