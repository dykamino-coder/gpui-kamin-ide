// session:activity → onSessionIdle debounce extracted from
// connection-manager.ts (Sprint 5 / Stage E1).
//
// Fires `onIdle` only on a real working→idle transition, debounced so a
// quick dip-then-resume doesn't trigger a "finished" toast. Also requires
// that we saw a real working spell within the last minute — otherwise the
// initial idle on connection would spam toasts.

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
  /** Сервер шлёт состояния жизненного цикла (`UserPromptSubmit`/`Stop`).
   *  Влияет ТОЛЬКО на то, сколько ждать подтверждения эвристического простоя:
   *  считать, что раз пришёл хоть один хук, то придёт и хук завершения витка,
   *  нельзя — в поле это неверно. */
  private hookSeen = false

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

  /**
   *  `hookDriven` — состояние пришло из хука жизненного цикла CLI, а не из
   *  эвристики заголовка OSC.
   *
   *  BR-23: оба вида состояний шли сюда неразличимо, а трекер не знает границ
   *  витка. Простой OSC-мигание «работает → простаивает» посреди оркестрации
   *  Agent Teams снова разрешало тост, и до ЕДИНСТВЕННОГО `Stop` главного витка
   *  успевало накопиться несколько «Session finished». Поэтому как только
   *  сервер показал, что умеет хуки, виток закрывает ТОЛЬКО хук: эвристический
   *  простой по-прежнему отслеживается, но тоста не даёт.
   */
  track(rawTitle: string | undefined, isWorking: boolean, hookDriven = false): void {
    if (hookDriven) this.hookSeen = true
    if (isWorking) {
      this.lastWorkingAt = Date.now()
      this.wasWorking = true
      if (this.debounceTimer) { clearTimeout(this.debounceTimer); this.debounceTimer = null }
      return
    }
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
      if (this.settleActive()) return // attach-replay blip — not a real turn
      if (Date.now() - this.lastWorkingAt > RECENT_WORK_WINDOW_MS) return
      this.onIdle(rawTitle ?? '')
    }, wait)
  }

  /** Снять висящий debounce при закрытии таба/сессии: иначе таймер стрелял
   *  тостом «Session finished» для уже закрытого таба (клик вёл в никуда). */
  dispose(): void {
    if (this.debounceTimer) { clearTimeout(this.debounceTimer); this.debounceTimer = null }
    this.wasWorking = false
  }
}
