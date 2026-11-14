import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { CliHandlerConditions, HOOK_CONDITION_STATE, hasCliHandlerConditions } from './cli-handler-conditions'
import { clearSession, registerSessionHooks } from './registry'

const directories: string[] = []
const gates: CliHandlerConditions[] = []
const ids = ['mine', 'other']
afterEach(() => {
  gates.splice(0).forEach((gate) => gate.dispose())
  directories.splice(0).forEach((directory) => fs.rmSync(directory, { recursive: true, force: true }))
  ids.forEach(clearSession)
})

function fixture() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-condition-test-'))
  directories.push(directory)
  const file = path.join(directory, 'settings.local.json')
  fs.writeFileSync(file, JSON.stringify({ env: { KEEP: 'original' }, permissions: { allow: ['Read'] } }))
  const gate = new CliHandlerConditions('mine', file, { cwd: '/client project', home: '/client home' })
  gates.push(gate)
  const read = () => JSON.parse(fs.readFileSync(file, 'utf8'))
  const state = () => JSON.parse(read().env[HOOK_CONDITION_STATE] || '{}')
  return { file, gate, read, state }
}

function declarations() {
  registerSessionHooks(
    'mine',
    {
      PreToolUse: [
        {
          hooks: [
            { type: 'prompt', prompt: 'original', if: 'Bash(echo *)' },
            { type: 'agent', prompt: 'original', if: 'Bash(git *)' },
            { type: 'mcp_tool', server: 'fixture', tool: 'Check', if: 'Read(~/notes/**)' },
            { type: 'command', command: 'original', if: 'Bash(pwd)' },
          ],
        },
      ],
    },
    { kind: 'user' },
  )
}

describe('CLI original-handler condition decisions', () => {
  it('supplies argument decisions for original handler types using client paths', async () => {
    declarations()
    const { gate, state, read } = fixture()
    expect(hasCliHandlerConditions('mine')).toBe(true)
    await gate.prepare(
      'PreToolUse',
      { tool_name: 'mcp__user-tools__Bash', tool_input: { command: 'echo fixture' } },
      '1',
    )
    expect(state().mine).toEqual({
      generation: '1',
      event: 'PreToolUse',
      rules: {
        'Bash(echo *)': true,
        'Bash(git *)': false,
        'Read(~/notes/**)': false,
      },
    })
    gate.restore('1')
    gate.release('1')
    await gate.prepare(
      'PreToolUse',
      { tool_name: 'mcp__user-tools__Read', tool_input: { file_path: '/client home/notes/a.md' } },
      '2',
    )
    expect(state().mine.rules['Read(~/notes/**)']).toBe(true)
    expect(read()).toMatchObject({ env: { KEEP: 'original' }, permissions: { allow: ['Read'] } })
  })

  it('holds parallel events until the previous declarations are restored and released', async () => {
    declarations()
    const { gate, state } = fixture()
    const payload = { tool_name: 'mcp__user-tools__Bash', tool_input: { command: 'echo fixture' } }
    await gate.prepare('PreToolUse', payload, '1')
    let ready = false
    const second = gate.prepare('PreToolUse', payload, '2').then(() => {
      ready = true
    })
    await Promise.resolve()
    expect(ready).toBe(false)
    expect(gate.restore('wrong')).toEqual({})
    gate.release('wrong')
    expect(gate.restore('1')).toEqual({ generation: '1:restore' })
    expect(state().mine).toEqual({ generation: '1:restore' })
    await Promise.resolve()
    expect(ready).toBe(false)
    gate.release('1')
    await second
    expect(state().mine.generation).toBe('2')
  })

  it('preserves markers belonging to another session and removes only its own on shutdown', async () => {
    declarations()
    const { file, gate, state, read } = fixture()
    const settings = read()
    settings.env[HOOK_CONDITION_STATE] = JSON.stringify({ other: { generation: 'live' } })
    fs.writeFileSync(file, JSON.stringify(settings))
    await gate.prepare('PreToolUse', { tool_name: 'mcp__user-tools__Bash', tool_input: { command: 'pwd' } }, '1')
    expect(state().other).toEqual({ generation: 'live' })
    gate.dispose()
    expect(state()).toEqual({ other: { generation: 'live' } })
  })

  it('drains queued requests on disposal and does not write markers for unrelated events', async () => {
    declarations()
    const { gate, state } = fixture()
    expect(await gate.prepare('Stop', {}, 'none')).toEqual({})
    expect(state()).toEqual({})
    await gate.prepare('PreToolUse', {}, '1')
    const queued = gate.prepare('PreToolUse', {}, '2')
    gate.dispose()
    await expect(queued).rejects.toThrow('disposed')
    expect(state()).toEqual({})
  })

  it('cancels a queued dispatch without publishing its stale declarations', async () => {
    declarations()
    const { gate, state } = fixture()
    await gate.prepare('PreToolUse', {}, '1')
    const controller = new AbortController()
    const queued = gate.prepare('PreToolUse', {}, 'cancelled', controller.signal)
    controller.abort()
    gate.release('1')
    await expect(queued).rejects.toThrow('cancelled')
    expect(state().mine.generation).toBe('1:restore')
    await expect(gate.prepare('PreToolUse', {}, 'next')).resolves.toEqual({ generation: 'next' })
  })

  it('ends an aborted turn, drains its queue, and ignores late cleanup from that turn', async () => {
    declarations()
    const { gate, state } = fixture()
    await gate.prepare('PreToolUse', {}, 'old')
    const queued = gate.prepare('PreToolUse', {}, 'old-queued')
    gate.reset()
    await expect(queued).rejects.toThrow('cancelled')
    expect(state()).toEqual({})
    await gate.prepare('PreToolUse', {}, 'new')
    expect(gate.restore('old')).toEqual({})
    gate.release('old')
    expect(state().mine.generation).toBe('new')
  })
})
