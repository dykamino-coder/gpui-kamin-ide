import assert from 'node:assert/strict'
import type { CapabilityItem } from './capability-inventory.js'
import { assertCapabilityMatrix, assertBrowserHostReady, type Matrix } from './capability-matrix.js'

// This floor is independent of provider declarations: deleting a prerequisite
// cannot turn the remaining owner acceptance into a successful completion.
export const completionContracts = [
  'host-transcript-reducers', 'host-message-queue', 'zero-page-tools-and-prompts',
  'terminal-query-compatibility', 'restart-files-settings-auth', 'agent-preview-hmr',
  'long-turn-and-reattach-metrics',
] as const
export const completionScenarios = [
  'image-contract', 'credential-storage', 'upstream-health', 'workspace-start', 'browser-login',
  'workspace-context', 'chat', 'file-tools', 'bash', 'mcp', 'hook', 'workspace-shell',
  'close-mid-turn', 'reopen-permission', 'two-browser-tabs', 'browser-metrics',
  'plugin-lsp', 'monitors', 'attachments-settings', 'restart-persistence',
  'preview-hmr', 'disconnect-shutdown',
] as const

export interface ExecutedProof {
  id: string
  status: 'PASS' | 'FAIL' | 'BLOCKED' | 'XFAIL' | 'SKIP'
  test: string
  assertion: string
}
export interface CompletionReceipt {
  schemaVersion: 1
  source: string
  sourceDirty: false
  imageSource: string
  imageId: string
  platform: 'linux'
  browserVersion: string
  exitCode: 0
  cleanup: []
  checks: ExecutedProof[]
}

export function requiredProofIds(matrix: Matrix) {
  return [
    ...matrix.rows.filter(row => row.status === 'supported-in-browser-host').map(row => `capability:${row.id}`),
    ...completionContracts.map(id => `contract:${id}`),
    ...completionScenarios.map(id => `live:${id}`),
  ].sort()
}

const object = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === 'object' && !Array.isArray(value)
const named = (value: unknown) => typeof value === 'string' && value.trim().length > 0

/** Validate executed evidence, never infer behavior from source/test-file presence.
 * A receipt is an execution artifact, not a replacement for its producing suites.
 * Only fixed reasons and reviewed inventory IDs may appear in failure output.
 */
export function fullParityGaps(inventory: CapabilityItem[], matrix: Matrix, receipt: unknown,
  candidate: string, dirty: boolean): string[] {
  assertCapabilityMatrix(inventory, matrix)
  assertBrowserHostReady(matrix)
  const gaps: string[] = []
  if (!/^[a-f0-9]{40}$/.test(candidate) || dirty) gaps.push('CANDIDATE_MUST_BE_CLEAN_COMMIT')
  const required = requiredProofIds(matrix)
  if (!object(receipt)) return [...gaps, 'EXECUTION_RECEIPT_MISSING', ...required.map(id => `MISSING ${id}`)]
  if (receipt.schemaVersion !== 1) gaps.push('RECEIPT_SCHEMA')
  if (receipt.source !== candidate || receipt.sourceDirty !== false || receipt.imageSource !== candidate) gaps.push('RECEIPT_SOURCE_MISMATCH')
  if (typeof receipt.imageId !== 'string' || !/^(sha256:)?[a-f0-9]{64}$/.test(receipt.imageId)) gaps.push('IMMUTABLE_IMAGE_REQUIRED')
  if (receipt.platform !== 'linux' || !named(receipt.browserVersion)) gaps.push('LINUX_BROWSER_EXECUTION_REQUIRED')
  if (receipt.exitCode !== 0 || !Array.isArray(receipt.cleanup) || receipt.cleanup.length !== 0) gaps.push('EXECUTION_OR_CLEANUP_FAILED')
  const checks = Array.isArray(receipt.checks) ? receipt.checks : []
  const seen = new Set<string>()
  for (const check of checks) {
    if (!object(check) || typeof check.id !== 'string' || !required.includes(check.id)) {
      gaps.push('UNKNOWN_PROOF_ID') // Do not echo arbitrary receipt text.
      continue
    }
    if (seen.has(check.id)) gaps.push(`DUPLICATE ${check.id}`)
    seen.add(check.id)
    if (check.status !== 'PASS' || !named(check.test) || !named(check.assertion)) gaps.push(`UNPROVEN ${check.id}`)
  }
  for (const id of required) if (!seen.has(id)) gaps.push(`MISSING ${id}`)
  return gaps
}

export function assertFullParity(inventory: CapabilityItem[], matrix: Matrix, receipt: unknown,
  candidate: string, dirty: boolean) {
  const gaps = fullParityGaps(inventory, matrix, receipt, candidate, dirty)
  assert.equal(gaps.length, 0, `Full browser parity remains OPEN (${gaps.length} gaps):\n${gaps.slice(0, 12).join('\n')}`)
}
