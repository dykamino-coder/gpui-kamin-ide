import { describe, expect, it } from 'vitest'
import { consumeMonitorOutput } from './plugin-monitors'
import { mergeNonSensitivePluginOptions } from './sync/plugin-snapshot'

describe('plugin harness safety boundaries', () => {
  it('never syncs a plaintext shadow of a sensitive option', () => {
    const options = mergeNonSensitivePluginOptions({
      userConfig: {
        token: { type: 'string', sensitive: true, default: 'must-not-sync' },
        endpoint: { type: 'string', default: 'https://default.example' },
      },
    }, {
      token: 'legacy-plaintext-secret',
      endpoint: 'https://saved.example',
      undeclared: 'not-in-schema',
    })
    expect(options).toEqual({ endpoint: 'https://saved.example' })
  })

  it('bounds monitor output that never terminates a line', () => {
    const emitted: string[] = []
    const remainder = consumeMonitorOutput('', 'x'.repeat(70 * 1024), line => emitted.push(line))
    expect(remainder).toBe('')
    expect(emitted).toHaveLength(1)
    expect(emitted[0]).toContain('unterminated line truncated')
    expect(emitted[0]!.length).toBeLessThan(66 * 1024)
  })

  it('preserves fragmented normal monitor lines', () => {
    const emitted: string[] = []
    let remainder = consumeMonitorOutput('', 'fir', line => emitted.push(line))
    remainder = consumeMonitorOutput(remainder, 'st\nsecond\r\npartial', line => emitted.push(line))
    expect(emitted).toEqual(['first', 'second'])
    expect(remainder).toBe('partial')
  })
})
