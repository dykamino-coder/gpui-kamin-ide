// Exercise real child processes/PTY/JSONL watcher and loopback HTTP, no Claude account.
import { afterEach, describe, expect, it } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import http from 'node:http'
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { once } from 'node:events'
import { fakeCliPath } from './dev-cli'
import { JsonlWatcher, computeProjectSlug } from './jsonl-watcher'
import { projectSlug, usageLayouts } from '../../../test-support/fixtures.mjs'
import { seedPluginFixture } from '../../../test-support/plugin-fixture.mjs'

const outputs = new WeakMap<ChildProcessWithoutNullStreams, () => string>()
const cleanups: (() => void | Promise<void>)[] = []
afterEach(async () => {
  for (const cleanup of cleanups.splice(0).reverse()) await cleanup()
})
function sandbox() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-fake-'))
  cleanups.push(() => fs.rmSync(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 50 }))
  const cwd = path.join(root, 'session')
  fs.mkdirSync(cwd)
  return {
    root,
    cwd,
    env: {
      ...process.env,
      NODE_ENV: 'test',
      BRIDGE_DEV_FAKE_CLI: '1',
      BRIDGE_FAKE_CLI_HOME: root,
      ANTHROPIC_API_KEY: '',
      CLAUDE_CODE_OAUTH_TOKEN: '',
    },
  }
}
function launch(s: ReturnType<typeof sandbox>, args: string[] = []) {
  const child = spawn(process.execPath, [fakeCliPath, ...args], { cwd: s.cwd, env: s.env, windowsHide: true })
  let output = ''
  let errors = ''
  child.stdout.on('data', (data) => (output += data))
  outputs.set(child, () => output)
  child.stderr.on('data', (data) => (errors += data))
  cleanups.push(async () => {
    if (child.exitCode === null && child.signalCode === null) {
      child.kill()
      await once(child, 'exit')
    }
  })
  return { child, output: () => output, errors: () => errors }
}
function entries(s: ReturnType<typeof sandbox>) {
  const dir = path.join(s.root, '.claude/projects', computeProjectSlug(s.cwd))
  const files = fs.existsSync(dir) ? fs.readdirSync(dir).filter((f) => f.endsWith('.jsonl')) : []
  return files.flatMap((f) =>
    fs
      .readFileSync(path.join(dir, f), 'utf8')
      .trim()
      .split('\n')
      .filter(Boolean)
      .map((l) => JSON.parse(l)),
  )
}
async function command(child: ChildProcessWithoutNullStreams, s: ReturnType<typeof sandbox>, text: string) {
  const count = () => (outputs.get(child)!().match(/turn complete|\"type\":\"result\"/g) ?? []).length
  const before = count()
  child.stdin.write(text + '\n')
  await expect.poll(count, { timeout: 10000 }).toBeGreaterThan(before)
  expect(entries(s).length).toBeGreaterThan(0)
}

describe('credential-free fake Claude', () => {
  it('accepts split bracketed paste from the real submitText coordinator', async () => {
    const s = sandbox()
    const cli = launch(s)
    cli.child.stdin.write('\x1b[20')
    cli.child.stdin.write('0~/fake-pressure\x1b[201~\r')
    await expect.poll(() => entries(s).some((e) => e.message?.usage?.input_tokens === 978010)).toBe(true)
  })

  it('rejects standalone use in production before writing state', async () => {
    const s = sandbox()
    s.env.NODE_ENV = 'production'
    const cli = launch(s)
    await once(cli.child, 'exit')
    expect(cli.child.exitCode).not.toBe(0)
    expect(cli.errors()).toContain('dev/test only')
    expect(entries(s)).toEqual([])
  })

  it('refuses non-loopback MCP URLs without needing network or credentials', async () => {
    const s = sandbox()
    fs.writeFileSync(
      path.join(s.cwd, '.mcp.json'),
      JSON.stringify({ mcpServers: { 'user-tools': { url: 'https://example.invalid/mcp', headers: {} } } }),
    )
    const cli = launch(s)
    cli.child.stdin.write('/fake-mcp\n')
    await expect.poll(() => cli.errors()).toContain('Only loopback')
    expect(entries(s).some((e) => e.message?.content?.[0]?.type === 'tool_result')).toBe(false)
  })
  it('matches short and hashed CLI project slugs', () => {
    for (const p of [path.resolve('x'), path.resolve('x'.repeat(250))])
      expect(projectSlug(p)).toBe(computeProjectSlug(p))
  })
  it('writes compact metadata before new usage, then resumes the exact conversation', async () => {
    const s = sandbox()
    const cli = launch(s)
    await command(cli.child, s, '/fake-pressure')
    await command(cli.child, s, '/compact')
    expect(entries(s).find((e) => e.subtype === 'compact_boundary').compactMetadata).toEqual({
      trigger: 'manual',
      preTokens: 978010,
      postTokens: 15098,
    })
    expect(entries(s).filter((e) => e.type === 'assistant')).toHaveLength(1)
    await command(cli.child, s, '/fake-next-usage')
    await expect.poll(() => entries(s).filter((e) => e.type === 'assistant').length).toBe(2)
    const id = entries(s)[0].sessionId
    cli.child.kill()
    await once(cli.child, 'exit')
    const resumed = launch(s, ['--resume', id])
    await command(resumed.child, s, '/fake-compact-missing')
    expect(new Set(entries(s).map((e) => e.sessionId))).toEqual(new Set([id]))
    const bounds = entries(s).filter((e) => e.subtype === 'compact_boundary')
    expect(bounds[1].compactMetadata.postTokens).toBeUndefined()
    expect(new Set(entries(s).map((e) => e.uuid)).size).toBe(entries(s).length)
  })
  it('isolates same-name subagents and emits idle/follow-up/stale-idle/report ordering', async () => {
    const a = sandbox()
    const b = sandbox()
    const ca = launch(a)
    const cb = launch(b)
    await command(ca.child, a, '/fake-agents 800')
    await command(cb.child, b, '/fake-team 800')
    const childFiles = (s: ReturnType<typeof sandbox>) => {
      const id = entries(s)[0].sessionId
      return path.join(s.root, '.claude/projects', computeProjectSlug(s.cwd), id, 'subagents')
    }
    await expect.poll(() => fs.existsSync(childFiles(a))).toBe(true)
    await expect.poll(() => fs.existsSync(childFiles(b))).toBe(true)
    const fa = fs.readdirSync(childFiles(a)).find((f) => f.endsWith('.jsonl'))!
    const fb = fs.readdirSync(childFiles(b)).find((f) => f.endsWith('.jsonl'))!
    expect(fa).not.toBe(fb)
    expect(
      JSON.parse(fs.readFileSync(path.join(childFiles(a), fa.replace('.jsonl', '.meta.json')), 'utf8')).agentType,
    ).toBe('worker')
    await expect
      .poll(() =>
        fs
          .readFileSync(path.join(childFiles(a), fa), 'utf8')
          .trim()
          .split('\n'),
      )
      .toHaveLength(800)
    await command(ca.child, a, '/fake-idle')
    await command(ca.child, a, '/fake-follow-up')
    await command(ca.child, a, '/fake-report')
    const parent = entries(a)
    expect(parent.filter((e) => e.message?.content?.[0]?.name === 'SendMessage')).toHaveLength(1)
    expect(
      parent.some((e) => typeof e.message?.content === 'string' && e.message.content.includes('2026-01-01T00:00:01')),
    ).toBe(true)
    expect(fs.readFileSync(path.join(childFiles(a), fa), 'utf8')).toContain('follow-up complete')
  })
  it('real watcher completes overlapping replay generations and keeps live tail', async () => {
    const s = sandbox()
    const cli = launch(s)
    await command(cli.child, s, '/fake-replay')
    await expect.poll(() => entries(s).length, { timeout: 10000 }).toBe(3001)
    const file = path.join(s.root, '.claude/projects', computeProjectSlug(s.cwd), entries(s)[0].sessionId + '.jsonl')
    const statuses: any[] = []
    const delivered: any[] = []
    const watcher = new JsonlWatcher(
      0,
      (rows) => {
        delivered.push(...rows)
        return true
      },
      (status) => statuses.push(status),
    )
    cleanups.push(() => watcher.stop())
    ;(watcher as any).filePath = file
    watcher.replayAll()
    watcher.replayAll()
    await expect.poll(() => statuses.some((s) => s.replayComplete), { timeout: 15000 }).toBe(true)
    await command(cli.child, s, '/fake-next-usage')
    await (watcher as any).checkForNewContent()
    expect(delivered.some((e) => e.message?.usage?.input_tokens === 17000)).toBe(true)
  }, 20000)
  it('uses actual loopback MCP/hook envelopes and records result only after delivery', async () => {
    const s = sandbox()
    const calls: any[] = []
    let release: (() => void) | undefined
    const server = http.createServer(async (req, res) => {
      let raw = ''
      for await (const chunk of req) raw += chunk
      const body = JSON.parse(raw)
      calls.push({ url: req.url, body, auth: req.headers.authorization })
      if (body.method === 'tools/call') await new Promise<void>((resolve) => (release = resolve))
      res.setHeader('Content-Type', 'application/json')
      res.end(
        JSON.stringify({
          jsonrpc: '2.0',
          id: body.id,
          result:
            body.method === 'tools/call' ? { content: [{ type: 'text', text: 'synthetic delivered result' }] } : {},
        }),
      )
    })
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
    cleanups.push(
      () =>
        new Promise<void>((resolve) => {
          server.closeAllConnections()
          server.close(() => resolve())
        }),
    )
    const port = (server.address() as any).port
    fs.mkdirSync(path.join(s.cwd, '.claude'))
    fs.writeFileSync(
      path.join(s.cwd, '.claude/settings.json'),
      JSON.stringify({
        hooks: {
          SessionStart: [
            { hooks: [{ args: ['-e', `fetch("http://127.0.0.1:${port}/api/hooks/test/SessionStart/1")`] }] },
          ],
        },
      }),
    )
    fs.writeFileSync(
      path.join(s.cwd, '.mcp.json'),
      JSON.stringify({
        mcpServers: {
          'user-tools': {
            url: `http://127.0.0.1:${port}/mcp/test`,
            headers: { Authorization: 'Bearer synthetic-test-only' },
          },
        },
      }),
    )
    const cli = launch(s)
    cli.child.stdin.write('/fake-mcp\n')
    await expect.poll(() => typeof release).toBe('function')
    expect(calls.map((c) => c.body.method).filter(Boolean)).toEqual([
      'initialize',
      'notifications/initialized',
      'tools/list',
      'tools/call',
    ])
    expect(entries(s).some((e) => e.message?.content?.[0]?.type === 'tool_result')).toBe(false)
    expect(calls[0].body.hook_event_name).toBe('SessionStart')
    release!()
    await expect.poll(() => entries(s).some((e) => e.message?.content?.[0]?.type === 'tool_result')).toBe(true)
    expect(entries(s).at(-1).message.content[0].content[0].text).toContain('synthetic delivered result')
    expect(calls.find((c) => c.body.method === 'tools/call').body.params.arguments.command).toContain(
      'tool-barrier.mjs',
    )
  })
  it.each(Object.keys(usageLayouts))('renders %s usage through a native PTY', async (layout) => {
    const s = sandbox()
    const pty = await import('node-pty')
    const proc = pty.spawn(process.execPath, [fakeCliPath, '/usage'], {
      cwd: s.cwd,
      cols: 120,
      rows: 50,
      env: { ...s.env, BRIDGE_FAKE_USAGE_LAYOUT: layout },
    })
    cleanups.push(async () => {
      const exit = new Promise<void>((resolve) => proc.onExit(() => resolve()))
      proc.write('/exit\r')
      await exit
    })
    let output = ''
    proc.onData((chunk) => (output += chunk))
    await expect
      .poll(() => output, { timeout: 10000 })
      .toContain(usageLayouts[layout as keyof typeof usageLayouts].split('\n')[0])
    if (layout === 'fable') {
      expect(output).toContain('Current week (Fable)')
      expect(output).toContain('promotion')
    }
  })
  it('stream-json mode emits init, assistant and result envelopes', async () => {
    const s = sandbox()
    const cli = launch(s, ['--output-format', 'stream-json'])
    await command(cli.child, s, 'synthetic')
    await expect.poll(() => cli.output()).toContain('"type":"result"')
    const output = cli
      .output()
      .trim()
      .split('\n')
      .map((line) => JSON.parse(line))
    expect(output[0]).toMatchObject({ type: 'system', subtype: 'init' })
    expect(output.find((e) => e.type === 'assistant').session_id).toBe(entries(s)[0].sessionId)
  })
  it.each(['stale-cache', 'changed', 'disabled', 'offline'])(
    'creates offline plugin fixture %s without altering an existing profile',
    (mode) => {
      const s = sandbox()
      const fixture = seedPluginFixture(s.root, mode)
      expect(JSON.parse(fs.readFileSync(path.join(fixture.cache, '.claude-plugin/plugin.json'), 'utf8')).version).toBe(
        '1.0.0',
      )
      expect(JSON.parse(fs.readFileSync(path.join(fixture.source, '.claude-plugin/plugin.json'), 'utf8')).version).toBe(
        '2.0.0',
      )
      expect(
        JSON.parse(fs.readFileSync(path.join(fixture.subclone, '.claude-plugin/plugin.json'), 'utf8')).version,
      ).toBe(mode === 'changed' ? '1.0.0' : '2.0.0')
      expect(() => seedPluginFixture(s.root, mode)).toThrow('existing plugin profile')
    },
    20000,
  )
})
