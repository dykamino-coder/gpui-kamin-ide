import { signal, effect } from '@preact/signals'
import type { StatFilter } from '../components/dashboard/StatsGrid'

export type Theme = 'dark' | 'light'
export const theme = signal<Theme>(
  (typeof localStorage !== 'undefined' ? localStorage.getItem('bridge-theme') as Theme : null) || 'dark'
)
effect(() => {
  document.documentElement.setAttribute('data-theme', theme.value)
  localStorage.setItem('bridge-theme', theme.value)
})
export function toggleTheme() { theme.value = theme.value === 'dark' ? 'light' : 'dark' }

export const isSidebarCollapsed = signal(true)
export function toggleSidebar() { isSidebarCollapsed.value = !isSidebarCollapsed.value }

// Selected token from the right-sidebar Users list. When null the
// dashboard shows the global Stats card; when set it shows that
// token's per-token stats + per-token sessions list (with download).
export const selectedTokenId = signal<string | null>(null)
export function selectTokenId(id: string | null) { selectedTokenId.value = id }

export const activeModal = signal<string | null>(null)
export function openModal(id: string) { activeModal.value = id }
export function closeModal() { activeModal.value = null }

export interface Notification { id: string; text: string; type: 'info' | 'success' | 'warning' | 'error'; timeout?: number }
export const notifications = signal<Notification[]>([])
let notifCounter = 0
export function showNotification(text: string, type: Notification['type'] = 'info', timeout = 3000) {
  const id = `notif-${++notifCounter}`
  notifications.value = [...notifications.value, { id, text, type, timeout }]
  if (timeout > 0) setTimeout(() => { notifications.value = notifications.value.filter(n => n.id !== id) }, timeout)
  return id
}
export function dismissNotification(id: string) { notifications.value = notifications.value.filter(n => n.id !== id) }

// Request list modal
export const showRequestList = signal(false)
export const requestListFilter = signal<StatFilter>({ label: 'All Requests' })
export function openRequestList(filter: StatFilter) {
  requestListFilter.value = filter
  showRequestList.value = true
}
export function closeRequestList() { showRequestList.value = false }
