// Reviewed behavior mapping; registration alone never earns execution evidence.
import assert from 'node:assert/strict'

export const monitorCases = Object.freeze({
  list: { ids: ['api:listMonitors', 'invoke:monitors:list'],
    assertion: 'Both browser pages list independently traced project-owned processes and their exact plugin metadata' },
  log: { ids: ['api:getMonitorLog', 'invoke:monitors:get-log'],
    assertion: 'Reattached browsers recover stdout/stderr and unterminated exit tails produced with zero pages; unknown IDs return no log' },
  output: { ids: ['api:onMonitorOutput', 'event:monitor:output'],
    assertion: 'Both browser pages receive exact stream/text/timestamp output and unsubscribing stops further delivery' },
  started: { ids: ['api:onMonitorStarted', 'event:monitor:started'],
    assertion: 'Both browser pages receive one exact started payload per independently owned project monitor' },
  status: { ids: ['api:onMonitorStatus', 'event:monitor:status'],
    assertion: 'Both browser pages receive the nonzero natural exit status while another project process remains alive' },
  stopped: { ids: ['api:onMonitorStopped', 'event:monitor:stopped'],
    assertion: 'Both browser pages observe tab-owned removal; closing a live tab and zero-page host shutdown terminate their traced processes' },
})
for (const value of Object.values(monitorCases)) { Object.freeze(value.ids); Object.freeze(value) }

export function monitorEvidence() {
  const states = new Map()
  return {
    async prove(name, assertions) {
      assert.ok(Object.hasOwn(monitorCases, name), 'Unknown monitor evidence case')
      assert.ok(!states.has(name), 'Duplicate monitor evidence case')
      assert.equal(typeof assertions, 'function', 'Executed assertions are required')
      states.set(name, 'RUNNING')
      try { await assertions(); states.set(name, 'PASS') }
      catch (error) { states.set(name, 'FAIL'); throw error }
    },
    finish() {
      assert.ok(Object.keys(monitorCases).every(name => states.get(name) === 'PASS'),
        'Monitor evidence is incomplete or failed')
      return Object.values(monitorCases).flatMap(({ ids, assertion }) => ids.map(id => ({
        id: `capability:${id}`, status: 'PASS', test: 'test/monitors.smoke.mjs', assertion,
      })))
    },
  }
}
