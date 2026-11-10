// Reviewed behavioral cases, separate from registration/source inventory credit.
import assert from 'node:assert/strict'

export const coreToolCases = Object.freeze({
  Write: ['writeFile', 'Written bytes match an independent filesystem read'],
  Edit: ['editFile', 'Replacement changes only the selected text on disk'],
  Read: ['readFile', 'Shared reader returns independently seeded file text'],
  Glob: ['globSearch', 'Scoped pattern finds the target and excludes another extension'],
  Grep: ['grepSearch', 'Scoped search returns the matching line and excludes nonmatches'],
  Bash: ['bashExecute', 'Foreground shell reports stdout, stderr and a real file side effect'],
  BashOutput: ['readBackgroundOutput', 'Background stdout and stderr are delivered once by the cursor'],
  KillShell: ['killBackgroundShell', 'The exact background process exits after shared-handler termination'],
  NotebookEdit: ['notebookEdit', 'Replace, insert and delete preserve independently checked notebook cells'],
  TodoWrite: ['todoWrite', 'Shared writer acknowledges the exact todo list'],
  TodoList: ['todoList', 'Shared reader returns the written list and its later empty replacement'],
})

export function coreToolEvidence() {
  const states = new Map()
  return {
    async prove(tool, assertions) {
      assert.ok(Object.hasOwn(coreToolCases, tool), 'Unknown core tool evidence case')
      assert.ok(!states.has(tool), 'Duplicate core tool evidence case')
      assert.equal(typeof assertions, 'function', 'Executed assertions are required')
      states.set(tool, 'RUNNING')
      try { await assertions(); states.set(tool, 'PASS') }
      catch (error) { states.set(tool, 'FAIL'); throw error }
    },
    finish() {
      assert.ok(Object.keys(coreToolCases).every(tool => states.get(tool) === 'PASS'),
        'Core tool evidence is incomplete or failed')
      return Object.entries(coreToolCases).flatMap(([tool, [handler, assertion]]) =>
        [`tool:${tool}`, `handler:${handler}`].map(id => ({
          id: `capability:${id}`, status: 'PASS',
          test: 'test/core-tools.smoke.mjs', assertion,
        })))
    },
  }
}
