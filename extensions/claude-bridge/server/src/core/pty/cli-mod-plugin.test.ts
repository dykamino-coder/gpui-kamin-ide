import { afterEach, describe, expect, it } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import { CLI_MOD_PLUGIN, CliModTransport } from './cli-mod-transport'
import { register as registerPlain } from '../../cli-mod/hooks/bridge.js'
import { register as registerConditions } from '../../cli-mod/hooks/bridge-conditions.js'

const TOOL_CALL_EVENTS = [
  'tool.call',
  'classic.PreToolUse',
  'classic.PostToolUse',
  'classic.PostToolUseFailure',
  'classic.PermissionRequest',
  'classic.PermissionDenied',
  'settings.read',
  'classic.ConfigChange',
]
const TRANSPORT_EVENTS = ['prompt.submit', 'session.append', 'session.start', 'turn.start', 'turn.complete']

function events(register: (on: any) => unknown): string[] {
  const seen: string[] = []
  register((event: string) => {
    seen.push(event)
  })
  return seen
}

const transports: CliModTransport[] = []
afterEach(() => transports.splice(0).forEach((t) => t.dispose()))

describe('CLI mod entries', () => {
  it('registers no per-tool-call handler for a session without hook conditions', () => {
    expect(events(registerPlain).sort()).toEqual([...TRANSPORT_EVENTS].sort())
    expect(JSON.parse(fs.readFileSync(path.join(CLI_MOD_PLUGIN, 'hooks', 'hooks.json'), 'utf8'))).toEqual({
      modules: ['./bridge.js'],
    })
  })

  it('adds the condition gates only in the conditions entry', () => {
    expect(events(registerConditions).sort()).toEqual([...TOOL_CALL_EVENTS, ...TRANSPORT_EVENTS].sort())
  })

  it('builds the conditions plugin from the same files inside the private directory and removes it', () => {
    const transport = new CliModTransport('session-a')
    transports.push(transport)
    const root = transport.conditionsPlugin()
    expect(transport.conditionsPlugin()).toBe(root)
    expect(path.dirname(root)).toBe(transport.directory)
    expect(JSON.parse(fs.readFileSync(path.join(root, 'hooks', 'hooks.json'), 'utf8'))).toEqual({
      modules: ['./bridge-conditions.js'],
    })
    for (const file of [
      '.claude-plugin/plugin.json',
      'hooks/bridge-conditions.js',
      'hooks/conditions.js',
      'hooks/transport.js',
    ])
      expect(fs.readFileSync(path.join(root, file), 'utf8')).toBe(
        fs.readFileSync(path.join(CLI_MOD_PLUGIN, file), 'utf8'),
      )
    expect(fs.existsSync(path.join(root, 'hooks', 'bridge.js'))).toBe(false)
    transport.dispose()
    expect(fs.existsSync(transport.directory)).toBe(false)
  })
})
