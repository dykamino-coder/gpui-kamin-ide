/**
 * Simple TTL cache for a single value.
 */
export class TtlCache<T> {
  private value: T | null = null
  private lastAt = 0

  constructor(private ttlMs: number) {}

  get(force = false): T | null {
    if (force || !this.value || (Date.now() - this.lastAt) >= this.ttlMs) return null
    return this.value
  }

  set(value: T): void {
    this.value = value
    this.lastAt = Date.now()
  }

  clear(): void {
    this.value = null
    this.lastAt = 0
  }
}
