import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import type { CapabilityItem } from './capability-inventory.js'

export const bridgeRoot = fileURLToPath(new URL('../../', import.meta.url))
const manifestRoot = path.join(bridgeRoot, 'workspace/capabilities')
const kinds = ['api', 'event', 'forward', 'handler', 'invoke', 'route', 'send', 'tool']
export interface Mapping {
  id: string
  status: 'supported-in-browser-host' | 'explicitly-not-applicable-with-reason'
  reason?: string; evidence?: string; provider?: string; requires?: string[]
  routes?: string[]; handler?: string; sources: Record<string, number>
}
interface Provider { file?: string; test?: string; contract: string; requires?: string }
interface Requirement { state: 'pending' | 'implemented'; owner: string; reason: string; file?: string; test?: string }
export function readMatrix() {
  const read = <T>(name: string): T => JSON.parse(fs.readFileSync(path.join(manifestRoot, `${name}.json`), 'utf8'))
  assert.deepEqual(fs.readdirSync(manifestRoot).filter(x => x.endsWith('.json')).sort(),
    [...kinds, 'providers', 'requirements'].map(x => `${x}.json`).sort(), 'Unreviewed manifest file')
  return { rows: kinds.flatMap(kind => read<Mapping[]>(kind)),
    providers: read<Record<string, Provider>>('providers'), requirements: read<Record<string, Requirement>>('requirements') }
}
export type Matrix = ReturnType<typeof readMatrix>

// These accepted contracts must survive edits to either providers or requirements.
// This list is a floor for the declared gate, not the owner's full parity scope.
const requiredContracts = ['host-terminal', 'title-regeneration', 'finished-toast-route', 'preview-hmr']
function assertRequiredContracts(matrix: Matrix) {
  for (const id of requiredContracts) assert.ok(matrix.requirements[id], `Missing required browser-host contract: ${id}`)
}

/** Completeness is checked against an independent AST inventory, never a generated allow-all map. */
export function assertCapabilityMatrix(inventory: CapabilityItem[], matrix: Matrix, base = bridgeRoot) {
  assertRequiredContracts(matrix)
  const ids = inventory.map(x => x.id).sort()
  const mapped = matrix.rows.map(x => x.id).sort()
  assert.equal(new Set(mapped).size, mapped.length, 'Duplicate capability mapping')
  assert.deepEqual(mapped, ids, `Unmapped/stale capability items: ${ids.filter(id => !mapped.includes(id)).join(', ')}`)
  const rows = new Map(matrix.rows.map(row => [row.id, row]))
  const exists = (file: string | undefined) => {
    assert.ok(file && !path.isAbsolute(file) && !file.split('/').includes('..'), 'Expected relative evidence path')
    assert.ok(fs.statSync(path.join(base, file)).isFile(), `Missing evidence ${file}`)
  }
  for (const [id, provider] of Object.entries(matrix.providers)) {
    assert.ok(provider.contract?.trim(), `Missing provider contract ${id}`)
    if (provider.requires) assert.ok(matrix.requirements[provider.requires], `Unknown provider prerequisite ${id}`)
    else { exists(provider.file); exists(provider.test) }
  }
  for (const [id, requirement] of Object.entries(matrix.requirements)) {
    assert.ok(requirement.owner?.trim() && requirement.reason?.trim(), `Undocumented prerequisite ${id}`)
    assert.ok(['pending', 'implemented'].includes(requirement.state), `Unknown prerequisite state ${id}`)
    if (requirement.state === 'implemented') { exists(requirement.file); exists(requirement.test) }
  }
  for (const item of inventory) {
    const row = rows.get(item.id)!
    const context = item.id
    const files = [...new Set(item.sources.map(source => source.file))].sort()
    assert.deepEqual(row.sources, Object.fromEntries(files.map(file => [file, item.sources.filter(s => s.file === file).length])),
      `Unreviewed source sites: ${context}`)
    assert.deepEqual(row.handler, item.handler, `Changed tool handler: ${context}`)
    if (item.kind === 'api') assert.deepEqual(row.routes, item.routes ?? [], `Changed API routes: ${context}`)
    else assert.equal(row.routes, undefined, `Routes belong to API members: ${context}`)
    assert.ok(['supported-in-browser-host', 'explicitly-not-applicable-with-reason'].includes(row.status), `Unclassified ${context}`)
    if (row.status === 'explicitly-not-applicable-with-reason') {
      assert.ok(row.reason?.trim() && row.reason.length > 15, `Missing exclusion reason: ${context}`)
      exists(row.evidence)
      assert.equal(row.provider, undefined, `Excluded item has a provider: ${context}`)
      assert.equal(row.requires, undefined, `Excluded item has pending support: ${context}`)
    } else {
      assert.equal(row.reason, undefined, `Supported item has exclusion reason: ${context}`)
      if (row.provider === 'routes') {
        assert.equal(item.kind, 'api', `Only API members delegate to routes: ${context}`)
        assert.ok(row.routes?.length, `Missing API route: ${context}`)
        const targets = row.routes.map(id => rows.get(id))
        assert.ok(targets.some(target => target?.status === 'supported-in-browser-host'), `API has no supported route: ${context}`)
        assert.deepEqual(row.requires ?? [], [...new Set(targets.flatMap(target => target?.requires ?? []))].sort(),
          `API prerequisites drifted: ${context}`)
      } else {
        assert.ok(row.provider && matrix.providers[row.provider], `Unknown provider: ${context}`)
        const required = matrix.providers[row.provider].requires
        if (required) assert.ok(row.requires?.includes(required), `Missing provider prerequisite: ${context}`)
        if (row.provider === 'shared-host' && ['handler', 'invoke', 'send', 'event'].includes(item.kind)) {
          assert.ok(item.sources.some(source => source.shared), `Not reachable from shared BridgeHost: ${context}`)
        }
      }
      for (const required of row.requires ?? []) assert.ok(matrix.requirements[required], `Unknown prerequisite: ${context}`)
    }
    if (item.kind === 'tool') {
      const handler = rows.get(`handler:${item.handler}`)
      assert.ok(handler, `Uninventoried tool handler: ${context}`)
      assert.equal(handler.status, row.status, `Tool/handler classification differs: ${context}`)
      assert.deepEqual(row.requires ?? [], handler.requires ?? [], `Tool/handler prerequisites differ: ${context}`)
    }
    if (item.kind === 'api' && row.status === 'explicitly-not-applicable-with-reason') {
      assert.ok((row.routes ?? []).every(id => rows.get(id)?.status === row.status), `Excluded API has supported channel: ${context}`)
    }
  }
}

/** Declared readiness cannot pass with a missing or pending required contract. */
export function assertBrowserHostReady(matrix: Matrix) {
  assertRequiredContracts(matrix)
  const pending = Object.entries(matrix.requirements).filter(([, requirement]) => requirement.state !== 'implemented')
  assert.equal(pending.length, 0, 'Browser-host parity remains OPEN:\n' + pending.map(([id, requirement]) =>
    `${id} (${requirement.owner}): ${requirement.reason}`).join('\n'))
}
