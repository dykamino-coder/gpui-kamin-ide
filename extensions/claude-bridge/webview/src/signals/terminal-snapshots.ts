// Console scrollback is a bounded, document-local remount cache. It is not
// durable across CEF recreation or app restart; the PTY owns the current screen.
export const TERMINAL_SNAPSHOT_BYTES = 256 * 1024
export const TERMINAL_SNAPSHOT_SLOTS = 32
const encoder = new TextEncoder()
interface Writer { dirty: boolean; serialize: (scrollback: number) => string }

export class TerminalSnapshotCache {
  private snapshots = new Map<string, string>()
  private writers = new Map<string, Writer>()
  private timer: ReturnType<typeof setInterval> | undefined

  get(tabId: string): string | undefined { return this.snapshots.get(tabId) }
  attach(tabId: string, serialize: (scrollback: number) => string) {
    const writer: Writer = { dirty: false, serialize }
    this.writers.set(tabId, writer)
    if (!this.timer) this.timer = setInterval(() => this.flush(), 10_000)
    return {
      markDirty: () => { if (this.writers.get(tabId) === writer) writer.dirty = true },
      detach: () => {
        if (this.writers.get(tabId) !== writer) return
        this.checkpoint(tabId, writer)
        this.writers.delete(tabId)
        this.stopIfIdle()
      },
    }
  }
  reset(tabId: string): void {
    this.snapshots.delete(tabId)
    const writer = this.writers.get(tabId)
    if (writer) writer.dirty = false
  }
  close(tabId: string): void {
    this.reset(tabId)
    this.writers.delete(tabId) // late parsed writes/cleanup cannot reinsert it
    this.stopIfIdle()
  }
  private stopIfIdle(): void {
    if (!this.writers.size && this.timer) { clearInterval(this.timer); this.timer = undefined }
  }
  private flush(): void { for (const [tabId, writer] of this.writers) this.checkpoint(tabId, writer) }
  private checkpoint(tabId: string, writer: Writer): void {
    if (!writer.dirty) return
    try {
      let data = writer.serialize(1000)
      // Never cut an ANSI sequence or Unicode surrogate in half. If scrollback
      // is too large, serialize just the screen; otherwise skip this checkpoint.
      if (encoder.encode(data).length > TERMINAL_SNAPSHOT_BYTES) data = writer.serialize(0)
      if (encoder.encode(data).length > TERMINAL_SNAPSHOT_BYTES) { this.snapshots.delete(tabId); writer.dirty = false; return }
      this.snapshots.delete(tabId)
      this.snapshots.set(tabId, data)
      while (this.snapshots.size > TERMINAL_SNAPSHOT_SLOTS) this.snapshots.delete(this.snapshots.keys().next().value!)
    } catch { /* keep last successful snapshot; retry on the next tick */ return }
    writer.dirty = false
  }
}
export const terminalSnapshots = new TerminalSnapshotCache()
