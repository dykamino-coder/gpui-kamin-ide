// Bounded retry for READ-ONLY start-up requests.
//
// An `invoke-reply` can be lost on its way back to the page (a hidden view
// refuses it, a CEF reload swallows it). Without a deadline `init()` then waits
// forever behind "Restoring your session…" (INC-2026-0065). A read can simply be
// asked again, so every attempt is its own request and the FIRST reply settles
// the call; a late reply of an earlier attempt is harmless.
//
// Only for idempotent reads. A mutating request must never be re-issued blindly
// (the write may have happened — BR-24); those keep their own reconciliation.

/** Delay before each re-ask; after the last one a final window is waited out. */
export const READ_RETRY_DELAYS_MS: readonly number[] = [2_000, 4_000, 8_000, 16_000]
/** How long the last attempt is given before the call is declared lost. */
export const READ_FINAL_WAIT_MS = 30_000

export class ReadTimeoutError extends Error {
  constructor(label: string) {
    super(`${label} did not answer after ${READ_RETRY_DELAYS_MS.length + 1} attempts`)
    this.name = 'ReadTimeoutError'
  }
}

export function readWithRetry<T>(
  label: string,
  read: () => Promise<T>,
  delays: readonly number[] = READ_RETRY_DELAYS_MS,
  finalWaitMs: number = READ_FINAL_WAIT_MS,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    let settled = false
    let timer: ReturnType<typeof setTimeout> | undefined
    const settle = (fn: () => void): void => {
      if (settled) return
      settled = true
      if (timer !== undefined) clearTimeout(timer)
      fn()
    }
    const attempt = (n: number): void => {
      if (settled) return
      // A rejected read is a real answer (the host replied with an error):
      // retrying would only repeat it.
      read().then(v => settle(() => resolve(v)), e => settle(() => reject(e)))
      const wait = n < delays.length ? delays[n]! : finalWaitMs
      timer = setTimeout(() => {
        if (n < delays.length) attempt(n + 1)
        else settle(() => reject(new ReadTimeoutError(label)))
      }, wait)
    }
    attempt(0)
  })
}
