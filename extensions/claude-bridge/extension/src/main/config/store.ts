// ConfigStore — KaminIDE VSIX.
//
// The Electron app persisted config via `electron-store` (electron/main/config/
// store.ts), which needs Electron's userData path. The VSIX persists the same
// shape to a JSON file in the extension's global-storage dir (passed in by the
// activator). Public API matches the original 1:1 so the copied IPC handlers
// and session callbacks work unchanged.
import fs from "fs"
import path from "path"
import crypto from "crypto"
import type { ConnectionConfig, SavedSession } from "../../shared/types"

const defaults: ConnectionConfig = { serverUrl: "ws://localhost:3456", token: "" }

export interface LayoutState {
  sidebarWidth?: number
  sidebarWidthRatio?: number
  filePanelVisible?: boolean
  filePanelWidth?: number
  filePanelWidthRatio?: number
  filePanelBottomVisible?: boolean
  filePanelSplit?: number
}

export interface NotificationsConfig {
  backgroundToasts: boolean
}

export interface TerminalPrefs {
  useConptyDll: boolean
}

interface StoreData {
  config: ConnectionConfig
  savedSessions: SavedSession[]
  savedSessionsByToken: Record<string, SavedSession[]>
  /** The user's standing instructions, keyed BY TOKEN (mirrors
   *  savedSessionsByToken): they belong to a workspace/account, so switching
   *  tokens must not carry someone else's prompt along. Sent with every
   *  session:create/resume and APPENDED to the technical system prompt. */
  basePromptByToken: Record<string, string>
  layout: LayoutState
  notifications: NotificationsConfig
  terminal: TerminalPrefs
  /** One-shot marker: the Electron Bridge's saved sessions have been imported. */
  migratedFromElectron?: boolean
}

const notificationsDefaults: NotificationsConfig = { backgroundToasts: true }
const terminalPrefsDefaults: TerminalPrefs = { useConptyDll: false }

function tokenKey(token: string | undefined): string {
  if (!token) return "no-token"
  return crypto.createHash("sha256").update(token).digest("hex").slice(0, 16)
}

export class ConfigStore {
  private data: StoreData
  private readonly file: string

  constructor(storageDir: string) {
    this.file = path.join(storageDir, "open-claude-bridge-config.json")
    this.data = this.load()
  }

  private load(): StoreData {
    const base: StoreData = {
      config: { ...defaults },
      savedSessions: [],
      savedSessionsByToken: {},
      basePromptByToken: {},
      layout: {},
      notifications: { ...notificationsDefaults },
      terminal: { ...terminalPrefsDefaults },
    }
    try {
      const raw = JSON.parse(fs.readFileSync(this.file, "utf8")) as Partial<StoreData>
      return { ...base, ...raw }
    } catch {
      return base
    }
  }

  private persist(): void {
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true })
      const tmp = `${this.file}.tmp`
      fs.writeFileSync(tmp, JSON.stringify(this.data, null, 2), "utf8")
      fs.renameSync(tmp, this.file)
    } catch {
      /* best-effort */
    }
  }

  getTerminalPrefs(): TerminalPrefs {
    return { ...terminalPrefsDefaults, ...this.data.terminal }
  }

  setTerminalPrefs(patch: Partial<TerminalPrefs>): void {
    this.data.terminal = { ...this.getTerminalPrefs(), ...patch }
    this.persist()
  }

  getNotifications(): NotificationsConfig {
    return { ...notificationsDefaults, ...this.data.notifications }
  }

  setNotifications(patch: Partial<NotificationsConfig>): void {
    this.data.notifications = { ...this.getNotifications(), ...patch }
    this.persist()
  }

  getLayout(): LayoutState {
    return this.data.layout ?? {}
  }

  setLayout(patch: Partial<LayoutState>): void {
    this.data.layout = { ...this.getLayout(), ...patch }
    this.persist()
  }

  get(): ConnectionConfig {
    return this.data.config
  }

  set(config: ConnectionConfig): void {
    this.data.config = config
    this.persist()
  }

  private currentKey(): string {
    return tokenKey(this.get().token)
  }

  /** The user's standing instructions for the CURRENT token ('' when unset).
   *  Keyed by token like savedSessions — a different account must not inherit
   *  someone else's prompt. */
  getBasePrompt(): string {
    return (this.data.basePromptByToken ?? {})[this.currentKey()] ?? ''
  }

  setBasePrompt(text: string): void {
    const map = { ...(this.data.basePromptByToken ?? {}) }
    const trimmed = text.trim()
    if (trimmed) map[this.currentKey()] = trimmed
    else delete map[this.currentKey()] // empty = no prompt, don't keep a blank
    this.data.basePromptByToken = map
    this.persist()
  }

  getSavedSessions(): SavedSession[] {
    const map = this.data.savedSessionsByToken ?? {}
    const key = this.currentKey()
    if (map[key] && map[key].length > 0) return map[key]
    const legacy = this.data.savedSessions ?? []
    if (legacy.length > 0) {
      this.data.savedSessionsByToken = { ...map, [key]: legacy }
      this.data.savedSessions = []
      this.persist()
      return legacy
    }
    return []
  }

  addSavedSession(session: SavedSession): void {
    const map = this.data.savedSessionsByToken ?? {}
    const key = this.currentKey()
    const existing = map[key] ?? []
    const filtered = existing.filter((s) => s.conversationId !== session.conversationId)
    filtered.unshift(session)
    this.data.savedSessionsByToken = { ...map, [key]: filtered.slice(0, 50) }
    this.persist()
  }

  removeSavedSession(conversationId: string): void {
    const map = this.data.savedSessionsByToken ?? {}
    const key = this.currentKey()
    const existing = map[key] ?? []
    this.data.savedSessionsByToken = { ...map, [key]: existing.filter((s) => s.conversationId !== conversationId) }
    this.persist()
  }

  /** One-shot migration: pull the Electron Bridge's saved sessions into this
   *  store so a user upgrading from the Electron app keeps their project/session
   *  list. Both stores use the identical `open-claude-bridge-config.json` shape
   *  and the SAME token-key hash (sha256(token)[:16]), so buckets merge directly
   *  by key. KaminIDE's own sessions win on conflict; the Electron ones are
   *  appended newest-first and capped at 50/token (parity with addSavedSession).
   *  Resume works because the transcripts live on the server, resolved by
   *  conversationId. Runs once (guarded); best-effort. */
  migrateFromElectron(): void {
    if (this.data.migratedFromElectron) return
    const appData = process.env.APPDATA
    const candidates = appData
      ? [
          path.join(appData, "open-claude-bridge-electron", "open-claude-bridge-config.json"),
          path.join(appData, "Electron", "open-claude-bridge-config.json"),
        ]
      : []
    const activityMs = (s: SavedSession): number => {
      const t = new Date(s.lastActivity).getTime()
      return Number.isFinite(t) ? t : Number(s.lastActivity) || 0
    }
    const map = { ...(this.data.savedSessionsByToken ?? {}) }
    let importedTokens = 0
    for (const file of candidates) {
      let raw: { config?: ConnectionConfig; savedSessionsByToken?: Record<string, SavedSession[]> }
      try {
        raw = JSON.parse(fs.readFileSync(file, "utf8")) as typeof raw
      } catch {
        continue // absent / unreadable — skip
      }
      // Adopt the Electron app's connection config (serverUrl + token) when this
      // KaminIDE install isn't signed in yet, so an upgrading user doesn't have
      // to re-enter their token on first launch.
      if (!this.data.config.token && raw.config?.token) {
        this.data.config = { ...this.data.config, ...raw.config }
      }
      const src = raw.savedSessionsByToken ?? {}
      for (const [key, sessions] of Object.entries(src)) {
        if (!Array.isArray(sessions) || sessions.length === 0) continue
        const byId = new Map<string, SavedSession>()
        for (const s of map[key] ?? []) if (s?.conversationId) byId.set(s.conversationId, s)
        for (const s of sessions) if (s?.conversationId && !byId.has(s.conversationId)) byId.set(s.conversationId, s)
        map[key] = [...byId.values()].sort((a, b) => activityMs(b) - activityMs(a)).slice(0, 50)
        importedTokens++
      }
    }
    this.data.savedSessionsByToken = map
    this.data.migratedFromElectron = true
    this.persist()
    if (importedTokens > 0) console.log(`[bridge] imported Electron saved sessions for ${importedTokens} token bucket(s)`)
  }
}
