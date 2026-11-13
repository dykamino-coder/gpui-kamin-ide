// session:activity → onSessionIdle debounce extracted from
// connection-manager.ts (Sprint 5 / Stage E1).
//
// Fires `onIdle` only on a real working→idle transition, debounced so a
// quick dip-then-resume doesn't trigger a "finished" toast. An initial idle
// without observed work never opens a turn.

const DEBOUNCE_MS = 500
/** Сколько простой обязан продержаться, чтобы ЭВРИСТИЧЕСКИЙ простой закрыл
 *  виток на сервере, который умеет хуки.
 *
 *  Прежняя правка BR-23 просто запрещала таким простоям закрывать виток — и
 *  на связке, где завершение витка приезжает только эвристикой, уведомление
 *  о завершении пропало целиком (проверено A/B на живом приложении: на 1.0.60
 *  тост есть, на 1.0.61 его нет). Источник состояния больше не решает, будет
 *  уведомление или нет; он решает лишь, СКОЛЬКО ждать подтверждения. Мигания
 *  во время оркестрации возвращаются в работу за доли секунды и до этого
 *  порога не доживают, а настоящий конец витка — доживает. */
const OSC_CONFIRM_MS = 4_000
// After a (re)connect the server replays cached status, which can include a
// stale `isWorking:true` for an already-idle session; the immediate working→idle
// that follows would fire a bogus "Session finished" toast on plain session open.
// Suppress idle FIRING (not tracking) for that attach spell. A later idle→work
// transition proves a fresh turn and is eligible even while replay is settling.
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
  private wasWorking = false
  private idleSeenSinceAttach = false
  private workStartedAfterIdle = false
  private settleUntil = 0
  private awaitingReplaySince = 0
  /** Сервер шлёт состояния жизненного цикла (`UserPromptSubmit`/`Stop`).
   *  Влияет ТОЛЬКО на то, сколько ждать подтверждения эвристического простоя:
   *  считать, что раз пришёл хоть один хук, то придёт и хук завершения витка,
   *  нельзя — в поле это неверно. */
  private hookSeen = false

  constructor(private onIdle: (rawTitle: string) => void) {}

  /** Call on every socket (re)open — suppresses a cached in-flight spell while
   *  replay settles. Observing idle followed by new work opens a fresh turn. */
  armSettle(): void {
    this.idleSeenSinceAttach = false
    this.workStartedAfterIdle = false
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

  /**
   *  `hookDriven` — состояние пришло из хука жизненного цикла CLI, а не из
   *  эвристики заголовка OSC.
   *
   *  Hook idle uses the ordinary debounce; OSC idle needs a longer confirmation
   *  after hooks have been seen, so short orchestration blips do not end a turn.
   */
  track(rawTitle: string | undefined, isWorking: boolean, hookDriven = false): void {
    if (hookDriven) this.hookSeen = true
    if (isWorking) {
      // A ready/idle report followed by new work is a fresh turn, even if its
      // first transcript replay finishes nearby. Capture this only when opening
      // the working spell: an idle blip during attach must not bless stale work.
      if (!this.wasWorking) this.workStartedAfterIdle = this.idleSeenSinceAttach
      this.wasWorking = true
      if (this.debounceTimer) { clearTimeout(this.debounceTimer); this.debounceTimer = null }
      return
    }
    this.idleSeenSinceAttach = true
    if (!this.wasWorking) return
    // Хук завершения авторитетен — ждём обычный debounce. Эвристический простой
    // на сервере с хуками обязан ПОДТВЕРДИТЬСЯ: любое возвращение в работу
    // сбрасывает таймер выше, поэтому мигание внутри витка тоста не даёт.
    const wait = this.hookSeen && !hookDriven ? OSC_CONFIRM_MS : DEBOUNCE_MS
    if (this.debounceTimer) clearTimeout(this.debounceTimer)
    this.debounceTimer = setTimeout(() => {
      this.debounceTimer = null
      if (!this.wasWorking) return
      this.wasWorking = false // consume the working spell either way
      if (this.settleActive() && !this.workStartedAfterIdle) return
      this.onIdle(rawTitle ?? '')
    }, wait)
  }

  /** Снять висящий debounce при закрытии таба/сессии: иначе таймер стрелял
   *  тостом «Session finished» для уже закрытого таба (клик вёл в никуда). */
  dispose(): void {
    if (this.debounceTimer) { clearTimeout(this.debounceTimer); this.debounceTimer = null }
    this.wasWorking = false
    this.idleSeenSinceAttach = false
    this.workStartedAfterIdle = false
  }
}
