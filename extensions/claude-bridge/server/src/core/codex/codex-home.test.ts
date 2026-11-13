import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { injectProxyEnv } from '../config/settings'
import {
  assertBridgeOwnedHome,
  buildCodexEnv,
  CODEX_CA_ENV,
  defaultCodexHome,
  migrateLegacyCodexHome,
  prepareCodexHome,
} from './codex-home'
import { patchModelCatalog } from './model-catalog'

// Type-only reference keeps the production injector's signature compatible.
const _sig: (env: Record<string, string>) => void = injectProxyEnv
void _sig

const roots: string[] = []
afterEach(() => {
  for (const r of roots.splice(0)) fs.rmSync(r, { recursive: true, force: true })
})
const tmp = () => {
  const r = fs.mkdtempSync(path.join(os.tmpdir(), 'codex-home-test-'))
  roots.push(r)
  return r
}

describe('CODEX_HOME', () => {
  it('defaults to one server-wide Bridge-owned home outside ~/.claude, overridable', () => {
    expect(defaultCodexHome({})).toBe(path.join(os.homedir(), '.kamin-bridge', 'codex'))
    expect(defaultCodexHome({}).startsWith(path.join(os.homedir(), '.claude'))).toBe(false)
    expect(defaultCodexHome({ BRIDGE_CODEX_HOME: '/data/codex' })).toBe('/data/codex')
  })

  it('refuses the operator personal ~/.codex', () => {
    const env = {}
    expect(() => assertBridgeOwnedHome('/home/op/.codex', '/home/op', env)).toThrow('operator ~/.codex')
    expect(() => assertBridgeOwnedHome('/home/op/.codex/sub', '/home/op', env)).toThrow('operator ~/.codex')
    expect(() => assertBridgeOwnedHome('/home/op/.codex-bridge', '/home/op', env)).not.toThrow()
    expect(() => assertBridgeOwnedHome('/home/op/.kamin-bridge/codex', '/home/op', env)).not.toThrow()
  })

  it('allows the Bridge user ~/.codex only inside the container image when named explicitly', () => {
    const container = { BRIDGE_CONTAINER: '1', BRIDGE_CODEX_HOME: '/home/bridge/.codex' }
    expect(() => assertBridgeOwnedHome('/home/bridge/.codex', '/home/bridge', container)).not.toThrow()
    expect(() => assertBridgeOwnedHome('/home/bridge/.codex/sub', '/home/bridge', container)).toThrow(
      'operator ~/.codex',
    )
    expect(() =>
      assertBridgeOwnedHome('/home/bridge/.codex', '/home/bridge', { BRIDGE_CODEX_HOME: '/home/bridge/.codex' }),
    ).toThrow('operator ~/.codex')
    expect(() => assertBridgeOwnedHome('/home/bridge/.codex', '/home/bridge', { BRIDGE_CONTAINER: '1' })).toThrow(
      'operator ~/.codex',
    )
  })

  it('moves a legacy ~/.claude/codex-home into the new home once', () => {
    const root = tmp()
    const legacy = path.join(root, '.claude', 'codex-home')
    const home = path.join(root, '.codex')
    fs.mkdirSync(path.join(legacy, 'sessions'), { recursive: true })
    fs.writeFileSync(path.join(legacy, 'auth.json'), '{"synthetic":true}')
    fs.writeFileSync(path.join(legacy, 'sessions', 'a.jsonl'), 'x')
    expect(migrateLegacyCodexHome(home, legacy)).toBe(true)
    expect(fs.readFileSync(path.join(home, 'auth.json'), 'utf8')).toBe('{"synthetic":true}')
    expect(fs.readFileSync(path.join(home, 'sessions', 'a.jsonl'), 'utf8')).toBe('x')
    expect(fs.existsSync(legacy)).toBe(false)
    // Second start: nothing left to move.
    expect(migrateLegacyCodexHome(home, legacy)).toBe(false)
  })

  it('never overwrites a home that already has an account', () => {
    const root = tmp()
    const legacy = path.join(root, '.claude', 'codex-home')
    const home = path.join(root, '.codex')
    fs.mkdirSync(legacy, { recursive: true })
    fs.mkdirSync(home, { recursive: true })
    fs.writeFileSync(path.join(legacy, 'auth.json'), '{"old":true}')
    fs.writeFileSync(path.join(home, 'auth.json'), '{"new":true}')
    expect(migrateLegacyCodexHome(home, legacy)).toBe(false)
    expect(fs.readFileSync(path.join(home, 'auth.json'), 'utf8')).toBe('{"new":true}')
    expect(fs.existsSync(path.join(legacy, 'auth.json'))).toBe(true)
  })

  it('writes config.toml and catalog.json, and leaves auth.json alone', () => {
    const home = path.join(tmp(), 'codex-home')
    fs.mkdirSync(home)
    fs.writeFileSync(path.join(home, 'auth.json'), '{"synthetic":true}')
    const catalog = patchModelCatalog({ models: [{ slug: 'gpt-5.5' }] })
    const prepared = prepareCodexHome(home, catalog)
    expect(JSON.parse(fs.readFileSync(prepared.catalogPath, 'utf8'))).toEqual(catalog)
    const toml = fs.readFileSync(prepared.configPath, 'utf8')
    expect(toml).toContain('cli_auth_credentials_store = "file"')
    expect(toml).toContain(`model_catalog_json = ${JSON.stringify(prepared.catalogPath)}`)
    expect(fs.readFileSync(path.join(home, 'auth.json'), 'utf8')).toBe('{"synthetic":true}')
    expect(fs.readdirSync(home).sort()).toEqual(['auth.json', 'catalog.json', 'config.toml'])
    if (process.platform !== 'win32') {
      expect(fs.statSync(home).mode & 0o777).toBe(0o700)
      expect(fs.statSync(prepared.configPath).mode & 0o777).toBe(0o600)
    }
    prepareCodexHome(home, catalog) // idempotent rewrite
    expect(fs.readdirSync(home).sort()).toEqual(['auth.json', 'catalog.json', 'config.toml'])
  })
})

describe('buildCodexEnv', () => {
  const base = {
    PATH: '/bin',
    TEMP: '/tmp',
    OPENAI_API_KEY: 'sk-synthetic',
    CODEX_API_KEY: 'synthetic',
    ANTHROPIC_API_KEY: 'synthetic',
    BRIDGE_ADMIN_TOKEN: 'synthetic',
    HOME: '/home/op',
  }

  it('passes only the allowlist and isolates HOME into the codex home', () => {
    const env = buildCodexEnv({ home: '/data/codex', base })
    expect(env).toEqual({
      PATH: '/bin',
      TEMP: '/tmp',
      CODEX_HOME: '/data/codex',
      HOME: '/data/codex',
      USERPROFILE: '/data/codex',
      RUST_LOG: 'warn',
    })
  })

  it('applies Bridge proxy settings through the shared injector, bypassing loopback', () => {
    const env = buildCodexEnv({
      home: '/h',
      base: { ...base, NO_PROXY: 'corp.local' },
      injectProxy: (e) => {
        e.HTTPS_PROXY = 'http://proxy.corp:3128'
        e.NODE_EXTRA_CA_CERTS = '/data/certs/corp.pem'
      },
    })
    expect(env.HTTPS_PROXY).toBe('http://proxy.corp:3128')
    expect(env.NO_PROXY).toBe('corp.local,127.0.0.1,localhost,::1')
    expect(env.no_proxy).toBe(env.NO_PROXY)
    // codex reads its extra trust roots from CODEX_CA_CERTIFICATE, not NODE_EXTRA_CA_CERTS.
    expect(env[CODEX_CA_ENV]).toBe('/data/certs/corp.pem')
  })

  it('gives inherited proxies the same loopback bypass', () => {
    const env = buildCodexEnv({ home: '/h', base: { ...base, https_proxy: 'http://p:1' } })
    expect(env.https_proxy).toBe('http://p:1')
    expect(env.NO_PROXY).toBe('127.0.0.1,localhost,::1')
  })

  it('leaves NO_PROXY alone without a proxy and keeps an explicit codex CA', () => {
    const env = buildCodexEnv({
      home: '/h',
      base: { ...base, NODE_EXTRA_CA_CERTS: '/a.pem', CODEX_CA_CERTIFICATE: '/b.pem' },
    })
    expect(env.NO_PROXY).toBeUndefined()
    expect(env[CODEX_CA_ENV]).toBe('/b.pem')
  })

  it('lets tests add a mock provider key', () => {
    expect(buildCodexEnv({ home: '/h', base, extra: { MOCK_KEY: 'k' } }).MOCK_KEY).toBe('k')
  })
})
