// Bounded ring buffer with O(1) append. Auto-evicts the oldest item when full.
//
// Replaces the `arr.push(x); if (arr.length > N) arr.shift()` pattern used in
// error-log, plugin-monitors, and test-log. `Array.shift()` is O(n) on a
// thousand-entry log because every remaining element must shift one slot left;
// at 500-line caps that is acceptable, but the pattern is duplicated 4× and
// drifts (one place uses 200, another 300, another 500). One primitive,
// configurable cap, predictable semantics.
//
// Storage uses a fixed-size backing array plus a head index — `add()` is
// O(1) regardless of size, `toArray()` reconstructs in insertion order.

export class CircularBuffer<T> {
  private readonly buffer: Array<T | undefined>
  private head = 0
  private size = 0

  constructor(public readonly capacity: number) {
    if (!Number.isInteger(capacity) || capacity <= 0) {
      throw new Error(`CircularBuffer capacity must be a positive integer, got ${capacity}`)
    }
    this.buffer = new Array<T | undefined>(capacity)
  }

  /** Append a single item. Evicts the oldest entry once at capacity. */
  add(item: T): void {
    this.buffer[this.head] = item
    this.head = (this.head + 1) % this.capacity
    if (this.size < this.capacity) this.size++
  }

  /** Append multiple items at once. */
  addAll(items: Iterable<T>): void {
    for (const item of items) this.add(item)
  }

  /** Snapshot of all items in insertion order, oldest → newest. */
  toArray(): T[] {
    if (this.size === 0) return []
    const out: T[] = new Array<T>(this.size)
    const start = this.size < this.capacity ? 0 : this.head
    for (let i = 0; i < this.size; i++) {
      out[i] = this.buffer[(start + i) % this.capacity] as T
    }
    return out
  }

  /** Most recent N items, ordered oldest → newest. Returns fewer than N when
   *  the buffer hasn't filled up yet. */
  recent(count: number): T[] {
    if (count <= 0 || this.size === 0) return []
    const take = Math.min(count, this.size)
    const out: T[] = new Array<T>(take)
    const start = this.size < this.capacity ? 0 : this.head
    const offset = this.size - take
    for (let i = 0; i < take; i++) {
      out[i] = this.buffer[(start + offset + i) % this.capacity] as T
    }
    return out
  }

  /** Drop all items but keep the allocated capacity. */
  clear(): void {
    for (let i = 0; i < this.capacity; i++) this.buffer[i] = undefined
    this.head = 0
    this.size = 0
  }

  /** Current item count (0..capacity). */
  get length(): number {
    return this.size
  }

  /** True once the buffer has reached its capacity. */
  get isFull(): boolean {
    return this.size === this.capacity
  }
}
