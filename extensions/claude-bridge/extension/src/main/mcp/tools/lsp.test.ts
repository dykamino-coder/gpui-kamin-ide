import path from 'node:path'
import { beforeEach, expect, it, vi } from 'vitest'

const providers = vi.hoisted(() => ({ native: vi.fn(), plugin: vi.fn() }))
vi.mock('vscode', () => ({
  languages: { getDiagnostics: providers.native },
  Uri: { file: (file: string) => ({ fsPath: file }) },
}))
vi.mock('../../plugin-lsp', () => ({ pluginLspManager: { getDiagnostics: providers.plugin } }))
import { lspDiagnostics } from './lsp'

beforeEach(() => { providers.native.mockReturnValue([]); providers.plugin.mockResolvedValue([]) })

it('keeps native zero-based and plugin one-based severities when combining diagnostics', async () => {
  const diagnostics = (source: string, first: number) => Array.from({ length: 4 }, (_, index) => ({
    severity: first + index, source, message: `${source}-${index}`, code: index,
    range: { start: { line: index, character: 2 } },
  }))
  const native = diagnostics('native', 0), plugin = diagnostics('plugin', 1)
  providers.native.mockReturnValue(native)
  providers.plugin.mockResolvedValue(plugin)
  const file = path.resolve('severity.proof')
  const result = await lspDiagnostics({ file }, { tabId: 'owner-tab' } as any)
  const labels = ['Error', 'Warning', 'Info', 'Hint']
  expect(result.content[0].text?.split('\n')).toEqual(['native', 'plugin'].flatMap(source =>
    labels.map((label, index) => `[${label}] ${file}:${index + 1}:3 ${source}-${index} (${source}) [${index}]`)))
  expect(providers.plugin).toHaveBeenCalledWith(file, 'owner-tab')
  expect(plugin.map(d => d.severity)).toEqual([1, 2, 3, 4])
})

it('defaults missing plugin severity to Error and preserves native fallback after plugin failure', async () => {
  const file = path.resolve('fallback.proof')
  providers.plugin.mockResolvedValue([{ message: 'unspecified', code: { value: 'fixture' } }])
  expect((await lspDiagnostics({ file })).content[0].text)
    .toBe(`[Error] ${file}:1:1 unspecified [{"value":"fixture"}]`)
  providers.plugin.mockRejectedValue(new Error('synthetic provider failure'))
  providers.native.mockReturnValue([{ severity: 1, message: 'native warning' }])
  expect((await lspDiagnostics({ file })).content[0].text).toBe(`[Warning] ${file}:1:1 native warning`)
  providers.native.mockReturnValue([])
  expect((await lspDiagnostics({ file })).content[0].text).toBe(`No diagnostics for ${file}.`)
})
