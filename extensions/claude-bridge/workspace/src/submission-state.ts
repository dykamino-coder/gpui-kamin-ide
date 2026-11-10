import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import type { SubmissionUpdate } from '../../webview/shared/submission-queue.js'

export interface DurableSubmission {
  tabId: string
  digest: string
  bytes: number
  receipt: SubmissionUpdate & { text: string }
}
export interface SubmissionState {
  submissions: DurableSubmission[]
  consumedRows: Array<[string, string[]]>
}
export interface SubmissionStorage {
  load(): SubmissionState
  save(state: SubmissionState): void
}

const MAX_TEXT = 32 * 1024 * 1024
const MAX_FILE = 256 * 1024 * 1024 // JSON escaping can expand retained text sixfold.
const STATES = new Set(['queued', 'sending', 'native', 'read', 'cancelled', 'uncertain', 'fallback'])
const text = (value: unknown, max: number): value is string => typeof value === 'string' && value.length > 0 && value.length <= max
const boolean = (value: unknown) => value === undefined || typeof value === 'boolean'

/** Private prompt data, never a reusable token. Invalid state cannot silently
 * remove the duplicate barrier and permit ambiguous delivery a second time. */
export class WorkspaceSubmissionState implements SubmissionStorage {
  private readonly file: string
  private readonly scope: string
  constructor(storage: string, root: string, serverUrl: string, token: string) {
    this.file = path.join(storage, 'workspace-submissions.json')
    this.scope = createHash('sha256').update(JSON.stringify([root, serverUrl, token])).digest('hex')
  }

  load(): SubmissionState {
    try {
      if (fs.statSync(this.file).size > MAX_FILE) throw new Error('oversized')
      const state = JSON.parse(fs.readFileSync(this.file, 'utf8'))
      if (state.version !== 1 || !/^[a-f0-9]{64}$/.test(state.scope)) throw new Error('invalid scope')
      if (state.scope !== this.scope) return { submissions: [], consumedRows: [] }
      return this.validate(state)
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { submissions: [], consumedRows: [] }
      throw new Error('Workspace submission state is unreadable; restore storage before starting')
    }
  }

  save(state: SubmissionState): void {
    const value = JSON.stringify({ version: 1, scope: this.scope, ...this.validate(state) })
    if (Buffer.byteLength(value) > MAX_FILE) throw new Error('Workspace submission state limit reached')
    const temporary = `${this.file}.tmp`
    const descriptor = fs.openSync(temporary, 'w', 0o600)
    try { fs.writeFileSync(descriptor, value); fs.fsyncSync(descriptor) }
    finally { fs.closeSync(descriptor) }
    fs.renameSync(temporary, this.file)
    // Linux container durability includes the directory entry after rename.
    if (process.platform !== 'win32') {
      const directory = fs.openSync(path.dirname(this.file), 'r')
      try { fs.fsyncSync(directory) } finally { fs.closeSync(directory) }
    }
  }

  private validate(state: SubmissionState): SubmissionState {
    if (!Array.isArray(state.submissions) || state.submissions.length > 10000 ||
        !Array.isArray(state.consumedRows) || state.consumedRows.length > 500) throw new Error('Invalid submission state')
    const ids = new Set<string>()
    let bytes = 0
    const submissions = state.submissions.map(item => {
      const receipt = item?.receipt
      if (!text(item?.tabId, 128) || !receipt || !text(receipt.id, 1024) || typeof receipt.text !== 'string' ||
          !Number.isSafeInteger(item.bytes) || item.bytes !== Buffer.byteLength(receipt.text) ||
          item.digest !== createHash('sha256').update(receipt.text).digest('hex') ||
          receipt.state !== undefined && !STATES.has(receipt.state) ||
          !boolean(receipt.cancelable) || !boolean(receipt.committed) ||
          !(receipt.uuid === undefined || text(receipt.uuid, 1024)) ||
          !(receipt.warning === undefined || typeof receipt.warning === 'string' && receipt.warning.length <= 4096) ||
          !(receipt.createdAt === undefined || Number.isSafeInteger(receipt.createdAt) && receipt.createdAt >= 0)) throw new Error('Invalid submission')
      const key = `${item.tabId}:${receipt.id}`
      bytes += item.bytes
      if (ids.has(key) || bytes > MAX_TEXT) throw new Error('Invalid submission identities')
      ids.add(key)
      const queue = receipt.fallbackQueue
      if (queue && (!Number.isSafeInteger(queue.afterOrd) || queue.afterOrd < -1 ||
          !Number.isSafeInteger(queue.enqueuedAt) || queue.enqueuedAt < 0 ||
          !boolean(queue.consumed) || !boolean(queue.dismissed))) throw new Error('Invalid queue receipt')
      return { tabId: item.tabId, digest: item.digest, bytes: item.bytes,
        receipt: { id: receipt.id, text: receipt.text, state: receipt.state, uuid: receipt.uuid,
          committed: receipt.committed, cancelable: receipt.cancelable, warning: receipt.warning, createdAt: receipt.createdAt,
          fallbackQueue: queue && { afterOrd: queue.afterOrd, enqueuedAt: queue.enqueuedAt, consumed: queue.consumed, dismissed: queue.dismissed } } }
    })
    const tabs = new Set<string>()
    let rows = 0
    const consumedRows: SubmissionState['consumedRows'] = state.consumedRows.map(item => {
      if (!Array.isArray(item) || item.length !== 2 || !text(item[0], 128) || tabs.has(item[0]) ||
          !Array.isArray(item[1]) || item[1].some(row => !text(row, 2048)) ||
          new Set(item[1]).size !== item[1].length || (rows += item[1].length) > 10000) throw new Error('Invalid consumed rows')
      tabs.add(item[0])
      return [item[0], [...item[1]]]
    })
    return { submissions, consumedRows }
  }
}
