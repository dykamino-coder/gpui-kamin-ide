// Reviewed behavior mapping; inventory registration alone earns no PASS.
import assert from 'node:assert/strict'

export const lspCases = Object.freeze({
  list: { ids: ['api:listLspServers', 'invoke:lsp:list'],
    assertion: 'Browser discovery returns the installed plugin and its real initialization state' },
  hover: { ids: ['tool:LspHover', 'handler:lspHover'],
    assertion: 'Zero-page hover returns seeded and changed document text; peer receives exact position and document versions' },
  definition: { ids: ['tool:LspDefinition', 'handler:lspDefinition'],
    assertion: 'Zero-page definition normalizes a LocationLink; peer receives exact file URI and position' },
  references: { ids: ['tool:LspReferences', 'handler:lspReferences'],
    assertion: 'Zero-page references normalize a Location and request declaration inclusion at the exact position' },
  diagnostics: { ids: ['tool:LspDiagnostics', 'handler:lspDiagnostics'],
    assertion: 'Zero-page diagnostics return updated document text, warning severity, source, code and one-based location' },
  restart: { ids: ['api:restartLspServers', 'invoke:lsp:restart'],
    assertion: 'Browser restart stops both project peers and subsequent tool requests initialize fresh isolated processes' },
})
for (const value of Object.values(lspCases)) { Object.freeze(value.ids); Object.freeze(value) }

export function lspEvidence() {
  const states = new Map()
  return {
    async prove(name, assertions) {
      assert.ok(Object.hasOwn(lspCases, name), 'Unknown LSP evidence case')
      assert.ok(!states.has(name), 'Duplicate LSP evidence case')
      assert.equal(typeof assertions, 'function', 'Executed assertions are required')
      states.set(name, 'RUNNING')
      try { await assertions(); states.set(name, 'PASS') }
      catch (error) { states.set(name, 'FAIL'); throw error }
    },
    finish() {
      assert.ok(Object.keys(lspCases).every(name => states.get(name) === 'PASS'),
        'LSP evidence is incomplete or failed')
      return Object.values(lspCases).flatMap(({ ids, assertion }) => ids.map(id => ({
        id: `capability:${id}`, status: 'PASS', test: 'test/lsp.smoke.mjs', assertion,
      })))
    },
  }
}
