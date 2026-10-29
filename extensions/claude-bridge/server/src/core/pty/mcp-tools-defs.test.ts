import { expect, it } from 'vitest'
import { MCP_TOOLS } from './mcp-tools-defs'

const tool = (name: string) => MCP_TOOLS.find((t) => t.name === name)!

it('advertises implemented Read and WebFetch behavior without unsupported parameters', () => {
  expect(tool('Read').description).toContain('PDF rendering is not implemented')
  expect(tool('Read').inputSchema.properties).not.toHaveProperty('pages')
  expect(tool('WebFetch').description).toContain('No separate model processes the prompt')
  expect(tool('WebFetch').description).toContain('private')
  expect(tool('Write').description).not.toContain('will fail if you did not read')
  expect(JSON.stringify(MCP_TOOLS)).not.toContain('dangerouslyDisableSandbox')
})

it('describes widgets as optional declarative visuals using actual theme tokens', () => {
  const widget = JSON.stringify(tool('ShowWidget'))
  expect(widget).not.toMatch(/freely|dark|inline js/i)
  for (const token of [
    '--text-primary',
    '--text-secondary',
    '--bg-surface',
    '--bg-overlay',
    '--accent-primary',
    '--accent-action',
  ])
    expect(widget).toContain(token)
  expect(widget).toContain('Script elements do not execute')
})
