// One shared source for every user-stats consumer of the admin page (right
// sidebar, user cards, stats cards grid). Before, each component polled
// /api/dashboard/users on its own timer and every stats card fetched its own
// overview; now one roster poll and one batched cards request feed them all.
//
// Unchanged entries keep their previous object identity, so memoized rows and
// cards skip re-rendering when a poll brings the same numbers.

import { effect, signal } from '@preact/signals'
import { api } from '../services/api-client'
import type { UserSummary } from '../services/api-client'
import { engineFilter, engineParam } from './engine'

export type StatsRange = 'all' | '30d' | '7d'

export interface CardData {
  sessions: number
  userMessages: number
  assistantMessages: number
  totalTokens: number
  favoriteModel: string | null
  models: Array<{ name: string; engine?: string; in: number; out: number; pct: number }>
  engines?: Partial<
    Record<
      'claude' | 'codex',
      {
        sessions: number
        userMessages: number
        assistantMessages: number
        inTokens: number
        outTokens: number
      }
    >
  >
  activeDays: number
  longestStreak: number
  peakHour: number | null
  /** [local date, user msgs, assistant msgs, tokens, sessions, {model: tokens}] */
  days: Array<[string, number, number, number, number, Record<string, number>]>
}

export const users = signal<UserSummary[]>([])
export const usersLoaded = signal(false)
export const usersError = signal<string | null>(null)
/** Cards for the default range, keyed by user name; '*' = all users. */
export const cards = signal<Record<string, CardData> | null>(null)
export const cardsError = signal<string | null>(null)
/** Percent of the one-time statistics build while it runs; null once ready. */
export const statsBuilding = signal<number | null>(null)

// While the build runs, poll every few seconds so progress moves and the real
// numbers appear as soon as it finishes.
let buildPoll: ReturnType<typeof setTimeout> | null = null
function noteBuilding(pct: number | null): void {
  const wasBuilding = statsBuilding.value != null
  statsBuilding.value = pct
  if (pct != null && !buildPoll && subscribers > 0) {
    buildPoll = setTimeout(() => {
      buildPoll = null
      void refreshUsers()
      if (cardsWanted > 0) void refreshCards()
    }, 3_000)
  } else if (pct == null && wasBuilding && cardsWanted > 0) {
    void refreshCards()
  }
}

function keepIdentity<T>(prev: T | undefined, next: T): T {
  return prev !== undefined && JSON.stringify(prev) === JSON.stringify(next) ? prev : next
}

let usersInFlight: Promise<void> | null = null
let generation = 0
export function refreshUsers(): Promise<void> {
  syncEngine()
  const engine = engineFilter.peek()
  const started = generation
  usersInFlight ??= api
    .getUsersWithBuild(engine)
    .then(({ list, building }) => {
      if (started !== generation || engine !== engineFilter.peek()) return
      noteBuilding(building)
      const prev = new Map(users.value.map((u) => [u.userName, u]))
      const next = list.map((u) => keepIdentity(prev.get(u.userName), u))
      const same = next.length === users.value.length && next.every((u, i) => u === users.value[i])
      if (!same) users.value = next
      usersError.value = null
    })
    .catch((e) => {
      if (started !== generation || engine !== engineFilter.peek()) return
      usersError.value = String(e)
    })
    .finally(() => {
      if (started !== generation || engine !== engineFilter.peek()) return
      usersLoaded.value = true
      usersInFlight = null
    })
  return usersInFlight
}

function zoneParams(): string {
  let tz = ''
  try {
    tz = Intl.DateTimeFormat().resolvedOptions().timeZone ?? ''
  } catch {
    /* old engine: offset only */
  }
  return `tz=${encodeURIComponent(tz)}&tzOffset=${new Date().getTimezoneOffset()}`
}

function authHeaders(): Record<string, string> {
  let tok: string | null = null
  try {
    tok = localStorage.getItem('dashboard_session_token')
  } catch {
    /* storage blocked */
  }
  return tok ? { Authorization: `Bearer ${tok}` } : {}
}

async function getCards(query: string): Promise<{
  unchanged?: boolean
  revision: string
  cards?: Record<string, CardData>
  building?: { pct: number }
}> {
  const res = await fetch(
    `/api/dashboard/stats/users-overview?${query}&${zoneParams()}${engineParam(engineFilter.peek())}`,
    {
      headers: authHeaders(),
      signal: AbortSignal.timeout(30_000),
    },
  )
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  return res.json()
}

let revision = ''
let cardsInFlight: Promise<void> | null = null
/** Default-range cards for everyone; a no-op round trip when nothing changed. */
export function refreshCards(): Promise<void> {
  syncEngine()
  const engine = engineFilter.peek()
  const started = generation
  cardsInFlight ??= getCards(`range=all&rev=${encodeURIComponent(revision)}`)
    .then((r) => {
      if (started !== generation || engine !== engineFilter.peek()) return
      cardsError.value = null
      if (r.building) {
        noteBuilding(r.building.pct)
        return
      }
      revision = r.revision
      if (!r.unchanged) noteBuilding(null)
      if (r.unchanged || !r.cards) return
      const prev = cards.value ?? {}
      const next: Record<string, CardData> = {}
      for (const [k, v] of Object.entries(r.cards)) next[k] = keepIdentity(prev[k], v)
      cards.value = next
    })
    .catch((e) => {
      if (started !== generation || engine !== engineFilter.peek()) return
      cardsError.value = String(e)
    })
    .finally(() => {
      if (started !== generation || engine !== engineFilter.peek()) return
      cardsInFlight = null
    })
  return cardsInFlight
}

/** One card in another range (a card's own All/30d/7d switch). */
export async function fetchCard(subject: string, range: StatsRange): Promise<CardData | null> {
  const r = await getCards(`range=${range}&subject=${encodeURIComponent(subject)}`)
  return r.cards?.[subject] ?? null
}

// Single poller shared by all mounted consumers; paused while the tab is hidden.
let subscribers = 0
let timer: ReturnType<typeof setInterval> | null = null
const tick = () => {
  if (typeof document !== 'undefined' && document.hidden) return
  void refreshUsers()
  if (cardsWanted > 0) void refreshCards()
}
let cardsWanted = 0
let stopEngineWatch: (() => void) | null = null
let observedEngine = engineFilter.peek()

function syncEngine(): void {
  const next = engineFilter.peek()
  if (next === observedEngine) return
  observedEngine = next
  generation++
  revision = ''
  usersInFlight = null
  cardsInFlight = null
  users.value = []
  usersLoaded.value = false
  cards.value = null
  usersError.value = null
  cardsError.value = null
}

/** Subscribe to the shared poll; returns the unsubscribe function. */
export function subscribeUserStats(opts: { cards?: boolean } = {}): () => void {
  subscribers++
  if (opts.cards) cardsWanted++
  if (!stopEngineWatch) {
    stopEngineWatch = effect(() => {
      engineFilter.value
      syncEngine()
      void refreshUsers()
      if (cardsWanted > 0) void refreshCards()
    })
  }
  if (!timer) timer = setInterval(tick, 15_000)
  void refreshUsers()
  if (opts.cards && !cards.value) void refreshCards()
  return () => {
    subscribers--
    if (opts.cards) cardsWanted--
    if (subscribers === 0 && timer) {
      clearInterval(timer)
      timer = null
      stopEngineWatch?.()
      stopEngineWatch = null
    }
  }
}
