// Хост владеет черновиками: CAS и атомарная запись предшествуют подтверждению.
import fs from 'node:fs'
import path from 'node:path'
import { createHash, randomUUID } from 'node:crypto'
import { emptyDraft, type WorkspaceDraft, type WorkspaceDraftState } from '../../webview/shared/workspace-draft.js'

const MAX_DRAFT = 8 * 1024 * 1024
const MAX_STORE = 32 * 1024 * 1024
const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v)
const idValid = (id: string) => /^[\w-]{1,128}$/.test(id) && !['__proto__', 'constructor', 'prototype'].includes(id)

function value(input: unknown): WorkspaceDraft {
  if (!object(input) || typeof input.text !== 'string' || Buffer.byteLength(input.text) > 256 * 1024 ||
      !Array.isArray(input.attachments) || input.attachments.length > 32 ||
      Object.keys(input).some(key => !['text', 'attachments'].includes(key))) throw new Error('Invalid workspace draft')
  const attachments = input.attachments.map(item => {
    if (!object(item) || !['path', 'name', 'ext'].every(key => typeof item[key] === 'string' && item[key].length <= 4096) ||
        !item.path || (item.dataUri !== undefined && (typeof item.dataUri !== 'string' || !item.dataUri.startsWith('data:image/'))) ||
        Object.keys(item).some(key => !['path', 'name', 'ext', 'dataUri'].includes(key))) throw new Error('Invalid draft attachment')
    return { path: item.path, name: item.name, ext: item.ext, ...(item.dataUri === undefined ? {} : { dataUri: item.dataUri }) }
  }) as WorkspaceDraft['attachments']
  const draft = { text: input.text, attachments }
  if (Buffer.byteLength(JSON.stringify(draft)) > MAX_DRAFT) throw new Error('Workspace draft exceeds 8 MiB')
  return draft
}

export class WorkspaceDrafts {
  private readonly file: string
  private readonly scope: string
  private states: Record<string, WorkspaceDraftState> = Object.create(null)
  constructor(storage: string, root: string, server: string, token: string) {
    this.file = path.join(storage, 'workspace-drafts.json')
    this.scope = createHash('sha256').update(JSON.stringify([root, server, token])).digest('hex')
    if (!fs.existsSync(this.file)) return
    if (fs.statSync(this.file).size > MAX_STORE) throw new Error('Workspace draft store exceeds 32 MiB')
    const saved: unknown = JSON.parse(fs.readFileSync(this.file, 'utf8'))
    if (!object(saved) || saved.version !== 1 || typeof saved.scope !== 'string') throw new Error('Invalid workspace draft store')
    if (saved.scope !== this.scope) return
    if (!object(saved.states) || Object.keys(saved.states).length > 128) throw new Error('Invalid workspace draft sessions')
    for (const [id, state] of Object.entries(saved.states)) {
      if (!idValid(id) || !object(state) || !Number.isSafeInteger(state.revision) || (state.revision as number) < 1) throw new Error('Invalid draft revision')
      this.states[id] = { revision: state.revision as number, value: value(state.value) }
    }
  }
  get(id: string): WorkspaceDraftState {
    if (!idValid(id)) throw new Error('Invalid draft session')
    return structuredClone(this.states[id] ?? { revision: 0, value: emptyDraft() })
  }
  update(id: string, revision: unknown, input: unknown) {
    const before = this.get(id)
    if (!Number.isSafeInteger(revision) || (revision as number) < 0) throw new Error('Invalid draft revision')
    const next = value(input)
    if (revision !== before.revision) return { accepted: false, state: before }
    if (before.revision >= Number.MAX_SAFE_INTEGER) throw new Error('Draft revision exhausted')
    const state = { revision: before.revision + 1, value: next }
    this.save({ ...this.states, [id]: state })
    return { accepted: true, state: structuredClone(state) }
  }
  prune(ids: string[]): void {
    const keep = new Set(ids)
    if (Object.keys(this.states).some(id => !keep.has(id))) this.save(Object.fromEntries(Object.entries(this.states).filter(([id]) => keep.has(id))))
  }
  private save(next: Record<string, WorkspaceDraftState>): void {
    if (Object.keys(next).length > 128) throw new Error('Too many workspace drafts')
    const data = JSON.stringify({ version: 1, scope: this.scope, states: next })
    if (Buffer.byteLength(data) > MAX_STORE) throw new Error('Workspace draft store exceeds 32 MiB')
    const temp = this.file + '.' + randomUUID() + '.tmp'
    try {
      const fd = fs.openSync(temp, 'wx', 0o600)
      try { fs.writeFileSync(fd, data); fs.fsyncSync(fd) } finally { fs.closeSync(fd) }
      fs.renameSync(temp, this.file)
      this.states = next
    } finally { if (fs.existsSync(temp)) fs.unlinkSync(temp) }
  }
}
