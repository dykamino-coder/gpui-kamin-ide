// Shared user color function.
// Colors assigned by index in sorted user list — guarantees maximum visual separation.
// Golden angle (~137.5°) spacing ensures any N users are spread across the full hue wheel.

import { computed } from '@preact/signals'
import { serverStats } from '../signals/server'

function hslToHex(h: number, s: number, l: number): string {
  s /= 100; l /= 100
  const a = s * Math.min(l, 1 - l)
  const f = (n: number) => {
    const k = (n + h / 30) % 12
    const c = l - a * Math.max(Math.min(k - 3, 9 - k, 1), -1)
    return Math.round(255 * c).toString(16).padStart(2, '0')
  }
  return `#${f(0)}${f(8)}${f(4)}`
}

/** Sorted user list derived from server stats — single source of truth */
export const sortedUsers = computed(() =>
  Object.keys(serverStats.value?.userRequests ?? {}).sort()
)

/** Get color for a user. Index in sorted list → golden angle hue. */
export function userColor(name: string): string {
  const users = sortedUsers.value
  let idx = users.indexOf(name)
  if (idx < 0) {
    // Fallback for users not yet in stats (e.g. chart API data arriving before WS)
    let h = 2166136261
    for (let i = 0; i < name.length; i++) h = Math.imul(h ^ name.charCodeAt(i), 16777619)
    idx = (h >>> 0) % 360
    return hslToHex(idx, 78, 60)
  }
  const hue = (idx * 137.508) % 360
  return hslToHex(hue, 78, 60)
}
