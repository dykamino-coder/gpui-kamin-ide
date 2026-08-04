// session:activity → onSessionIdle debounce extracted from
// connection-manager.ts (Sprint 5 / Stage E1).
//
// Fires `onIdle` only on a real working→idle transition, debounced so a
// quick dip-then-resume doesn't trigger a "finished" toast. Also requires
// that we saw a real working spell within the last minute — otherwise the
// initial idle on connection would spam toasts.

const DEBOUNCE_MS = 500
const RECENT_WORK_WINDOW_MS = 60_000
// After a (re)connect the server replays cached status, which can include a
// stale `isWorking:true` for an already-idle session; the immediate working→idle
// that follows would fire a bogus "Session finished" toast on plain session open.
// Suppress idle FIRING (not tracking) for a short window after `armSettle()`.
//
// Фикс C17 (аудит #70): фиксированных 4с не хватало реплею КРУПНОЙ сессии
// (десятки секунд) — блип прилетал после окна и тостил «finished» на ровном
// месте. Окно теперь держится ДО replayComplete (noteReplayComplete) с
// жёстким капом, плюс короткая грация после него на хвост статус-флаша.
const SETTLE_MS = 4_000
const SETTLE_HARD_CAP_MS = 60_000
const POST_REPLAY_GRACE_MS = 1_500

export class SessionIdleTracker {
  private debounceTimer: ReturnType<typeof setTimeout> | null = null
  private lastWorkingAt = 0
  private wasWorking = false
  private settleUntil = 0
  private awaitingReplaySince = 0

  constructor(private onIdle: (rawTitle: string) => void) {}

  /** Call on every socket (re)open — starts a window during which an idle
   *  transition won't fire a toast (covers the attach-replay blip). */
  armSettle(): void {
    this.settleUntil = Date.now() + SETTLE_MS
    this.awaitingReplaySince = Date.now()
  }

  /** Реплей закрыт сервером — снять «ждём реплей» и дать грацию на хвост. */
  noteReplayComplete(): void {
    this.awaitingReplaySince = 0
    this.settleUntil = Math.max(this.settleUntil, Date.now() + POST_REPLAY_GRACE_MS)
  }

  private settleActive(): boolean {
    if (Date.now() < this.settleUntil) return true
    return (
      this.awaitingReplaySince > 0
      && Date.now() - this.awaitingReplaySince < SETTLE_HARD_CAP_MS
    )
  }

  track(rawTitle: string | undefined, isWorking: boolean): void {
    if (isWorking) {
      this.lastWorkingAt = Date.now()
      this.wasWorking = true
      if (this.debounceTimer) { clearTimeout(this.debounceTimer); this.debounceTimer = null }
      return
    }
    if (!this.wasWorking) return
    if (this.debounceTimer) clearTimeout(this.debounceTimer)
    this.debounceTimer = setTimeout(() => {
      this.debounceTimer = null
      if (!this.wasWorking) return
      this.wasWorking = false // consume the working spell either way
      if (this.settleActive()) return // attach-replay blip — not a real turn
      if (Date.now() - this.lastWorkingAt > RECENT_WORK_WINDOW_MS) return
      this.onIdle(rawTitle ?? '')
    }, DEBOUNCE_MS)
  }

  /** Снять висящий debounce при закрытии таба/сессии: иначе таймер стрелял
   *  тостом «Session finished» для уже закрытого таба (клик вёл в никуда). */
  dispose(): void {
    if (this.debounceTimer) { clearTimeout(this.debounceTimer); this.debounceTimer = null }
    this.wasWorking = false
  }
}
