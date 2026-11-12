import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { describe, expect, it, vi } from 'vitest'
import { transcriptEngine } from '../pty/transcript-engine'
import { classifyCodexError, describeCodexError, loginRequired } from './codex-errors'
import { VirtualPty } from './virtual-pty'

describe('classifyCodexError', () => {
  it.each([
    ['unexpected status 407 Proxy Authentication Required', 'proxy-auth'],
    ['error sending request: invalid peer certificate: UnknownIssuer', 'tls'],
    ['self-signed certificate in certificate chain', 'tls'],
    ['unexpected status 401 Unauthorized: token expired', 'unauthorized'],
    ['Your access token could not be refreshed: refresh_token reused', 'unauthorized'],
    ['stream disconnected before completion: Connection refused (os error 111)', 'network'],
    ['dns error: failed to lookup address information', 'network'],
    ['model gpt-x is not supported', 'other'],
  ])('%s → %s', (text, kind) => {
    expect(classifyCodexError(text).kind).toBe(kind)
  })

  it('names the fix and keeps the original text', () => {
    const info = classifyCodexError('unexpected status 407 Proxy Authentication Required')
    expect(info.message).toContain('Bridge network settings')
    expect(describeCodexError(info)).toBe(`${info.message} (unexpected status 407 Proxy Authentication Required)`)
    expect(describeCodexError(classifyCodexError('plain failure'))).toBe('plain failure')
    expect(loginRequired().message).toContain('dashboard')
  })
})

describe('VirtualPty', () => {
  it('interrupts on Ctrl+C, stops once on kill and reports the exit once', async () => {
    const stop = vi.fn(async () => 0)
    const interrupt = vi.fn()
    const pty = new VirtualPty({ interrupt, stop }, 80, 24)
    const exits: number[] = []
    pty.onExit(({ exitCode }) => exits.push(exitCode))
    pty.write('hello')
    pty.write('\x03')
    expect(interrupt).toHaveBeenCalledTimes(1)
    pty.resize(100, 30)
    expect([pty.cols, pty.rows]).toEqual([100, 30])
    pty.kill()
    pty.kill('SIGKILL')
    await vi.waitFor(() => expect(exits).toEqual([0]))
    expect(stop).toHaveBeenCalledTimes(1)
    pty.markExited(5)
    pty.write('\x03')
    expect(interrupt).toHaveBeenCalledTimes(1)
    const late: number[] = []
    pty.onExit(({ exitCode }) => late.push(exitCode))
    await vi.waitFor(() => expect(late).toEqual([0]))
  })

  it('reports an engine that died on its own', () => {
    const pty = new VirtualPty({ interrupt: () => {}, stop: async () => 0 })
    const exits: number[] = []
    pty.onExit(({ exitCode }) => exits.push(exitCode))
    pty.markExited(101)
    expect(exits).toEqual([101])
    expect(pty.exited).toBe(true)
  })
})

describe('transcriptEngine', () => {
  it('tells codex projections from Claude transcripts', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'engine-'))
    try {
      const codex = path.join(dir, 'a.jsonl')
      const claude = path.join(dir, 'b.jsonl')
      fs.writeFileSync(codex, '\n{"type":"user","engine":"codex"}\n')
      fs.writeFileSync(claude, '{"type":"user","message":{"role":"user","content":"x"}}\n')
      expect(transcriptEngine(codex)).toBe('codex')
      expect(transcriptEngine(claude)).toBe('claude')
      expect(transcriptEngine(path.join(dir, 'missing.jsonl'))).toBeNull()
      fs.writeFileSync(claude, '')
      expect(transcriptEngine(claude)).toBeNull()
    } finally {
      fs.rmSync(dir, { recursive: true, force: true })
    }
  })
})
