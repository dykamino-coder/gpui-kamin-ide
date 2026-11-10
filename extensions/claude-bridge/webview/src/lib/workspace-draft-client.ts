// CAS-клиент черновиков: неподтверждённые записи не воспроизводятся после reconnect.
import { emptyDraft, type WorkspaceDraft, type WorkspaceDraftReply, type WorkspaceDraftState } from '../../shared/workspace-draft'
export type DraftStatus = 'loading' | 'saved' | 'saving' | 'conflict' | 'unsaved'
interface Entry {
  state: WorkspaceDraftState; loaded: boolean; desired?: WorkspaceDraft; serial: number
  flight?: { base: number; value: WorkspaceDraft }; blocked: boolean; status: DraftStatus
  listeners: Set<(value: WorkspaceDraft) => void>
}
interface Calls {
  get(id: string): Promise<WorkspaceDraftState>
  update(id: string, revision: number, value: WorkspaceDraft): Promise<WorkspaceDraftReply>
}
const equal = (a: WorkspaceDraft, b: WorkspaceDraft) => JSON.stringify(a) === JSON.stringify(b)

export class WorkspaceDraftClient {
  private entries = new Map<string, Entry>()
  constructor(private calls: Calls, private changed: () => void = () => {}) {}
  private entry(id: string): Entry {
    let entry = this.entries.get(id)
    if (!entry) {
      entry = { state: { revision: 0, value: emptyDraft() }, loaded: false, serial: 0,
        blocked: false, status: 'loading', listeners: new Set() }
      this.entries.set(id, entry)
    }
    return entry
  }
  status(id: string): DraftStatus { return this.entry(id).status }
  watch(id: string, apply: (value: WorkspaceDraft) => void): () => void {
    const entry = this.entry(id)
    entry.listeners.add(apply)
    if (entry.loaded) apply(structuredClone(entry.desired ?? entry.state.value))
    else void this.read(id)
    return () => { entry.listeners.delete(apply) }
  }
  private emit(entry: Entry): void {
    for (const listener of entry.listeners) listener(structuredClone(entry.state.value))
  }
  observe(id: string, state: WorkspaceDraftState): void {
    const entry = this.entries.get(id)
    if (!entry || state.revision <= entry.state.revision) return
    const own = entry.flight && state.revision === entry.flight.base + 1 && equal(state.value, entry.flight.value)
    entry.state = state
    if (entry.desired === undefined) { this.emit(entry); entry.status = 'saved' }
    else if (!own) { entry.blocked = true; entry.status = 'conflict' }
    this.changed()
  }
  save(id: string, value: WorkspaceDraft): void {
    const entry = this.entry(id)
    if (entry.desired !== undefined && equal(value, entry.desired)) return
    if (entry.loaded && entry.desired === undefined && equal(value, entry.state.value)) return
    entry.desired = structuredClone(value)
    entry.serial++
    if (!entry.blocked) entry.status = 'saving'
    this.changed()
    void this.pump(id, entry)
  }
  resolve(id: string, keepLocal: boolean): void {
    const entry = this.entry(id)
    if (!entry.loaded || entry.flight) return
    entry.blocked = false
    if (!keepLocal) { entry.desired = undefined; entry.status = 'saved'; this.emit(entry) }
    else { entry.status = 'saving'; void this.pump(id, entry) }
    this.changed()
  }
  private async pump(id: string, entry: Entry): Promise<void> {
    if (!entry.loaded || entry.flight || entry.blocked || entry.desired === undefined) return
    const serial = entry.serial
    const flight = { base: entry.state.revision, value: structuredClone(entry.desired) }
    entry.flight = flight
    try {
      const result = await this.calls.update(id, flight.base, flight.value)
      this.observe(id, result.state)
      if (!result.accepted) { entry.blocked = true; entry.status = 'conflict' }
      else if (!entry.blocked && serial === entry.serial) {
        entry.desired = undefined
        entry.status = 'saved'
      }
    } catch { entry.blocked = true; entry.status = 'unsaved' }
    finally {
      entry.flight = undefined
      this.changed()
      if (!entry.blocked && entry.desired !== undefined) void this.pump(id, entry)
    }
  }
  private async read(id: string): Promise<void> {
    const entry = this.entry(id)
    try {
      const state = await this.calls.get(id)
      if (state.revision >= entry.state.revision) entry.state = state
      entry.loaded = true
      if (entry.desired === undefined || equal(entry.desired, entry.state.value)) {
        entry.desired = undefined; entry.blocked = false; entry.status = 'saved'; this.emit(entry)
      } else { entry.blocked = true; entry.status = 'conflict' }
    } catch { entry.status = 'unsaved'; entry.blocked = true }
    this.changed()
  }
  refresh(): void { for (const id of this.entries.keys()) void this.read(id) }
  retain(ids: string[]): void {
    const keep = new Set(ids)
    for (const [id, entry] of this.entries) if (!keep.has(id) && !entry.listeners.size) this.entries.delete(id)
  }
  disconnected(): void {
    for (const entry of this.entries.values()) {
      if (entry.desired !== undefined) { entry.blocked = true; entry.status = 'unsaved' }
    }
    this.changed()
  }
}
