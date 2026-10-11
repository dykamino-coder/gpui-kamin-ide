// Verify fail-closed selection and release packaging, including forced dev env.
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { pathToFileURL } from 'node:url'
import { execFileSync } from 'node:child_process'
import { devCli, fakeCliPath, waitForDevCliStartup } from './dev-cli'
const home = os.homedir()
const enabled = { NODE_ENV: 'development', BRIDGE_DEV_FAKE_CLI: '1', BRIDGE_FAKE_CLI_HOME: home }

describe('checkout-only fake CLI selection', () => {
  it.each(['production', '', 'staging'])('cannot select fake in mode %s', (mode) => {
    expect(devCli({ ...enabled, NODE_ENV: mode })).toBeUndefined()
  })
  it('requires explicit opt-in, a matching isolated home and a shipped dev asset', () => {
    expect(devCli({ NODE_ENV: 'development' })).toBeUndefined()
    expect(() => devCli({ ...enabled, BRIDGE_FAKE_CLI_HOME: '' })).toThrow('isolated')
    expect(() => devCli({ ...enabled, BRIDGE_FAKE_CLI_HOME: path.join(home, 'different') })).toThrow('share')
    expect(() => devCli(enabled, false)).toThrow('release artifacts')
    expect(devCli(enabled)?.prefix).toEqual([fakeCliPath])
    expect(fs.existsSync(fakeCliPath)).toBe(true)
  })
  it('cancels delayed startup without reaching spawn', async () => {
    const controller = new AbortController()
    const waiting = waitForDevCliStartup(
      devCli({ ...enabled, BRIDGE_FAKE_CLI_STARTUP_DELAY_MS: '30000' }),
      controller.signal,
    )
    controller.abort()
    await expect(waiting).rejects.toThrow()
    expect(() => devCli({ ...enabled, BRIDGE_FAKE_CLI_STARTUP_DELAY_MS: 'NaN' })).toThrow('delay')
  })
  it('npm release excludes harness even if NODE_ENV is changed after installation', () => {
    // npm pack exercises the actual publish allowlist, rather than mocking it.
    const server = path.resolve(path.dirname(fakeCliPath), '..')
    const npmCli = process.env.npm_execpath
    expect(npmCli).toBeTruthy()
    const result = execFileSync(process.execPath, [npmCli!, 'pack', '--dry-run', '--json', '--ignore-scripts'], {
      cwd: server,
      encoding: 'utf8',
    })
    const files = JSON.parse(result)[0].files.map((f: { path: string }) => f.path)
    expect(files).toContain('src/core/pty/dev-cli.ts')
    expect(files.some((f: string) => f.startsWith('test-support/'))).toBe(false)
    expect(() => devCli(enabled, files.includes('test-support/fake-claude.mjs'))).toThrow('release artifacts')
  })

  it('a release-layout subprocess stays unreachable even with a forced development mode', () => {
    const release = fs.mkdtempSync(path.join(os.tmpdir(), 'fake-release-'))
    try {
      const module = path.join(release, 'src/core/pty/dev-cli.ts')
      fs.mkdirSync(path.dirname(module), { recursive: true })
      fs.copyFileSync(new URL('./dev-cli.ts', import.meta.url), module)
      fs.writeFileSync(path.join(release, 'package.json'), '{"type":"module"}')
      const server = path.resolve(path.dirname(fakeCliPath), '..')
      const loader = pathToFileURL(path.join(server, 'node_modules/tsx/dist/loader.mjs')).href
      const code = `const { devCli } = await import(${JSON.stringify(pathToFileURL(module).href)}); try { devCli(); process.exitCode=1 } catch(e) { if(!e.message.includes('release artifacts')) throw e }`
      execFileSync(process.execPath, ['--import', loader, '--input-type=module', '-e', code], {
        env: { ...process.env, ...enabled },
        stdio: 'pipe',
      })
      const production = `const { devCli } = await import(${JSON.stringify(pathToFileURL(module).href)}); if(devCli()!==undefined) process.exitCode=1`
      execFileSync(process.execPath, ['--import', loader, '--input-type=module', '-e', production], {
        env: { ...process.env, ...enabled, NODE_ENV: 'production' },
        stdio: 'pipe',
      })
    } finally {
      fs.rmSync(release, { recursive: true, force: true })
    }
  }, 20000)
  it('Docker runtime copies only allowlisted paths, with production mode', () => {
    const docker = fs
      .readFileSync(path.resolve(path.dirname(fakeCliPath), '../Dockerfile'), 'utf8')
      .split('AS runtime')[1]!
    expect(docker).toContain('ENV NODE_ENV=production')
    const sources = [...docker.matchAll(/^COPY --from=builder[^\n]+/gm)].map((m) => m[0])
    expect(sources.length).toBeGreaterThan(0)
    for (const source of sources) {
      expect(source).not.toMatch(/test-support|\/app\s/)
    }
    expect(sources.some((s) => s.includes('/app/src ./src'))).toBe(true)
  })
  it('session and usage spawn both use the guarded selector', () => {
    const core = fs.readFileSync(new URL('./session-core.ts', import.meta.url), 'utf8')
    expect(core).toContain('const developmentCli = devCli()')
    expect(core).toContain('waitForDevCliStartup(developmentCli, signal)')
    expect(core).toContain('developmentCli?.prefix')
    const usage = fs.readFileSync(new URL('../server/utils/usage-capture.ts', import.meta.url), 'utf8')
    expect(usage).toContain('const developmentCli = devCli()')
    expect(usage).toContain('developmentCli.prefix')
  })
})
