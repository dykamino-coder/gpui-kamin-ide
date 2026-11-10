import assert from 'node:assert/strict'

// Deliberately bounded: these IDs describe only the assertions in the browser
// fixture, not OAuth, other transports, every input or composed model acceptance.
export const mcpBrowserCases = {
  status: ['api:getMcpStatus', 'invoke:mcp:status'],
  list: ['api:getMcpServers', 'invoke:mcp:get-servers'],
  add: ['api:addMcpServer', 'invoke:mcp:add-server', 'api:onMcpServersChanged', 'event:mcp-servers-changed'],
  catalog: ['api:getMcpResources', 'invoke:mcp:get-resources', 'api:getMcpPrompts', 'invoke:mcp:get-prompts'],
  resource: ['api:readMcpResource', 'invoke:mcp:read-resource'],
  prompt: ['api:getMcpPrompt', 'invoke:mcp:get-prompt'],
  test: ['api:testMcpServer', 'invoke:mcp:test-server'],
  logs: ['api:getMcpTestLog', 'invoke:mcp:get-test-log', 'api:onMcpTestLog', 'event:mcp:test-log'],
  clear: ['api:clearMcpTestLog', 'invoke:mcp:clear-test-log', 'api:onMcpTestLogCleared', 'event:mcp:test-log-cleared'],
  progress: ['api:onMcpProgress', 'event:mcp-progress'],
  update: ['api:updateMcpServer', 'invoke:mcp:update-server'],
  toggle: ['api:toggleMcpServer', 'invoke:mcp:toggle-server'],
  remove: ['api:removeMcpServer', 'invoke:mcp:remove-server'],
  zeroPage: [],
}
for (const ids of Object.values(mcpBrowserCases)) Object.freeze(ids)
Object.freeze(mcpBrowserCases)

export function mcpBrowserEvidence() {
  const started = new Set(), passed = new Set()
  return {
    async prove(name, assertions) {
      assert.ok(Object.hasOwn(mcpBrowserCases, name), 'Unknown MCP case')
      assert.equal(typeof assertions, 'function', 'An executed assertion callback is required')
      assert.ok(!started.has(name), 'Duplicate MCP case')
      started.add(name)
      await assertions()
      passed.add(name)
    },
    finish() {
      assert.equal(passed.size, Object.keys(mcpBrowserCases).length, 'Missing or failed MCP case')
      return {
        cases: [...passed],
        checks: Object.entries(mcpBrowserCases).flatMap(([name, ids]) => ids.map(id => ({
          id, status: 'PASS', test: 'workspace/test/mcp-browser.smoke.mjs',
          assertion: `Executed browser MCP case: ${name}`,
        }))),
      }
    },
  }
}
