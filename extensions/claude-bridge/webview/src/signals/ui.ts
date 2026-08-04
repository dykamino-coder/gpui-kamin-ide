import { storage } from "@bridge/storage"
import { computed, signal } from '@preact/signals'

export type SidebarMode = 'sessions' | 'customize'
export type CustomizePanel = 'landing' | 'settings' | 'skills' | 'agents' | 'mcp' | 'plugins' | 'monitors' | 'sync' | 'logs' | 'hooks' | 'stats' | null

// True until the first `useInit` pass finishes restoring persisted tabs/state.
// Gates the main panel's empty view: while restoring we show a "Restoring…" cue
// instead of the "New session" welcome, so a returning user whose session is
// still loading doesn't think it's gone (and doesn't click New session by
// mistake). Flipped false in useInit's finally.
export const initializing = signal<boolean>(true)
export const sidebarMode = signal<SidebarMode>('sessions')
export const activeCustomizePanel = signal<CustomizePanel>(null)
export const currentEffort = signal('high')
// Default model for fresh sessions (matches the server's DEFAULT_SESSION_MODEL).
// Opus 5: 1M контекст нативно (как Fable 5) — [1m]-вариантов больше нет;
// 4.8 выпилен целиком (как раньше 4.7). См. contextLimitForModel.
export const DEFAULT_MODEL_ID = 'claude-opus-5'
export const currentModel = signal(DEFAULT_MODEL_ID)
export const currentPermission = signal('bypassPermissions')

export interface FileAttachment {
  path: string
  name: string
  ext: string
  dataUri?: string // only for images
}
/** @deprecated alias */
export type ImageAttachment = FileAttachment
export const pendingAttachments = signal<FileAttachment[]>([])

export const tabActivity = signal<Map<string, { rawTitle: string; isWorking: boolean }>>(new Map())

/** УЗКИЙ срез tabActivity: только boolean «работает». Читатели, которым не
 *  нужен текст спиннера (InputBar/QueueWidget/SessionItem/TabsBar), раньше
 *  ререндерились на каждый косметический тик rawTitle (~1Гц на рабочую
 *  вкладку) — Map заменялась целиком (аудит #70 D7). Возвращаем ПРЕЖНЮЮ
 *  ссылку, пока ни один isWorking не флипнулся: signals сравнивают по `===`,
 *  и подписчики не будятся. */
let prevWorking: Map<string, boolean> = new Map()
export const tabWorking = computed<Map<string, boolean>>(() => {
  const src = tabActivity.value
  let changed = src.size !== prevWorking.size
  if (!changed) {
    for (const [id, a] of src) {
      if (prevWorking.get(id) !== a.isWorking) {
        changed = true
        break
      }
    }
  }
  if (!changed) return prevWorking
  const out = new Map<string, boolean>()
  for (const [id, a] of src) out.set(id, a.isWorking)
  prevWorking = out
  return out
})

// Per-tab "waiting for the user" — set by the deterministic Notification hook
// (permission prompt, idle nudge, a question the CLI is blocked on), cleared on
// the next UserPromptSubmit/Stop/SessionStart. Distinct from isWorking so the
// sidebar dot can tell "still thinking" from "needs YOU".
export const tabWaiting = signal<Map<string, boolean>>(new Map())

// Terminal (xterm console) visibility — persisted to storage.
// `viewerVisible` is the legacy name that actually gates the terminal.
// First-run default is OFF — most users don't need the raw xterm console.
const _initialViewerVisible = storage.getItem('jsonl-viewer-visible') === 'true'
export const viewerVisible = signal<boolean>(_initialViewerVisible)

// JSONL viewer panel visibility — independent toggle.
const _initialJsonlVisible = storage.getItem('jsonl-panel-visible') !== 'false'
export const jsonlVisible = signal<boolean>(_initialJsonlVisible)

// Installed Claude CLI skills — populated on init. Feeds slash-command autocomplete.
export interface SkillInfo { name: string; description: string }
export const installedSkills = signal<SkillInfo[]>([])

// Server path + conversation IDs row visibility (persisted).
// First-run default is OFF — power-user diagnostic, not needed by default.
const _initialServerPathVisible = storage.getItem('server-path-visible') === 'true'
export const serverPathVisible = signal<boolean>(_initialServerPathVisible)

// Scroll-to-bottom tracking per visible panel. `true` = pinned at bottom.
// Seeded as true so the "Scroll down" pill stays hidden on fresh mount.
export const chatAtBottom = signal<boolean>(true)
export const terminalAtBottom = signal<boolean>(true)

// Auto-update: server version probe vs. local app version. When the server
// has a newer build, the sidebar "v<ver>" label turns into an "Update" button.
export interface UpdateInfo {
  localVersion: string
  serverVersion: string
  state: 'idle' | 'downloading' | 'ready' | 'error'
  error?: string
}
export const updateInfo = signal<UpdateInfo | null>(null)

// File-preview column — between chat and the right activity panel.
// Top region shows file content/details; bottom region shows a tooling
// pane (console / git history / etc). Both visibilities + width + split
// persist via main/config/store.ts (read on init in App, written
// debounced from the panel itself). Defaults are reasonable for first-
// run: closed by default, comfortable widths.
export const filePanelVisible = signal<boolean>(false)
export const filePanelWidth = signal<number>(380)
export const filePanelBottomVisible = signal<boolean>(false)
export const filePanelSplit = signal<number>(0.6)

// Sidebar visibility — toggleable from the titlebar quick-actions row.
// Persisted; defaults to visible.
const _initialSidebarVisible = storage.getItem('sidebar-visible') !== 'false'
export const sidebarVisible = signal<boolean>(_initialSidebarVisible)
export function setSidebarVisible(v: boolean): void {
  sidebarVisible.value = v
  try { storage.setItem('sidebar-visible', String(v)) } catch { /* noop */ }
}

/** Effective sidebar width that side-panel resize calculations should
 *  use. Returns the live `sidebarWidth` if the sidebar is actually on
 *  screen (visible-toggle on, OR customize mode forces it open), else
 *  0 — so file/right panels don't subtract a phantom sidebar from the
 *  available chat-min budget when the user has it collapsed. */
export function getEffectiveSidebarWidth(): number {
  const shown = sidebarVisible.value || sidebarMode.value === 'customize'
  return shown ? sidebarWidth.value : 0
}

// Sidebar width — Sidebar.tsx writes its current px width here so the
// RightPanel resize logic can subtract it from the viewport when
// re-balancing on window resize. Both panels share the side-panels area
// so neither can starve the chat column. Sidebar.tsx still owns the
// drag/persist behaviour; this is just a read mirror.
export const sidebarWidth = signal<number>(270)

// Right-hand activity panel — Plan (top) + Files (bottom) with a
// draggable horizontal splitter. Persisted so the layout survives
// relaunch. Width is clamped by CSS min/max; split ratio 0..1.
const _initialRightPanelVisible = storage.getItem('right-panel-visible') === 'true'
export const rightPanelVisible = signal<boolean>(_initialRightPanelVisible)
// Right-panel width — only a floor (300px so labels don't truncate). The
// user can drag arbitrarily wide; RightPanel.tsx caps each drag tick at
// window.innerWidth - chat-min, which scales with the monitor.
const _initialRightPanelWidth = Number(storage.getItem('right-panel-width') || '320')
export const rightPanelWidth = signal<number>(
  Number.isFinite(_initialRightPanelWidth)
    ? Math.max(300, _initialRightPanelWidth)
    : 320,
)
const _initialRightPanelSplit = Number(storage.getItem('right-panel-split') || '0.55')
export const rightPanelSplit = signal<number>(
  Number.isFinite(_initialRightPanelSplit) && _initialRightPanelSplit > 0 && _initialRightPanelSplit < 1
    ? _initialRightPanelSplit
    : 0.55,
)

// Bottom card in the right panel can show either Plan or Todos.
// Persisted across relaunches via storage.
export type BottomCardTab = 'plan' | 'todos'
const _initialBottomTab = storage.getItem('right-panel-bottom-tab')
export const rightPanelBottomTab = signal<BottomCardTab>(
  _initialBottomTab === 'todos' ? 'todos' : 'plan',
)

// User-pinned chat title colors. Key = sessionTitle (so the colour follows the
// agent name across resume/reopen), value = hex from AGENT_PALETTE. Persisted.
const _pinnedTitleColorsRaw = storage.getItem('pinned-title-colors')
let _pinnedTitleColors: Record<string, string> = {}
try { _pinnedTitleColors = _pinnedTitleColorsRaw ? JSON.parse(_pinnedTitleColorsRaw) : {} } catch { /* ignore */ }
export const pinnedTitleColors = signal<Record<string, string>>(_pinnedTitleColors)
export function setPinnedTitleColor(key: string, color: string | null): void {
  const next = { ...pinnedTitleColors.value }
  if (color) next[key] = color
  else delete next[key]
  pinnedTitleColors.value = next
  storage.setItem('pinned-title-colors', JSON.stringify(next))
}

// Token gate: true only when bridge.getConfig returned a non-empty token.
// Seeded as undefined (= "still loading") so the gate modal doesn't flash on
// mount before useInit reads the config.
export const hasToken = signal<boolean | undefined>(undefined)
