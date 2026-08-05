// Binds KaminIDE-native sessions ↔ Claude conversations (no duplicate session
// list). When a KaminIDE session becomes active, resume/create its Claude
// conversation and reveal the chat panel; bind the conversationId + AI-generated
// title back onto the session (metadata/name) so the native session row owns the
// identity. See docs/BRIDGE_VSIX_INTEGRATION.md.
import * as kaminide from "kaminide"
import type { KaminSession } from "kaminide"
import type { BridgeHost } from "./bridge-host"
import { ipcMain } from "@kaminide/host-compat"
import { ConnectionManager } from "./main/ws/connection-manager"
import { setSessionActivateHandler } from "./session-activation"
import { getDisplayedTab, onDisplayedTabChange } from "./displayed-tab"

interface BridgeMeta { conversationId?: string; cwd?: string | null }

// Warm pool: how long after install/config the OTHER open sessions are resumed
// in the background, and how many at most. The delay keeps the boot bandwidth
// (active session's connect + MCP discovery + first replay) for the session the
// user is looking at; the cap keeps a large fleet from spawning a CLI each.
const WARM_RESTORE_DELAY_MS = 4000
const WARM_POOL_MAX = 4
// Webview pre-fill: paint each warm tab's transcript into the webview store
// from the LOCAL DISK MIRROR — one session at a time, spaced out — so even the
// FIRST switch renders instantly. No server involved: warm tabs don't connect
// at boot at all (the boot replay storm was the exthost stall), the WS+CLI
// attach lazily on first switch.
const PREFILL_START_MS = 8000
const PREFILL_STAGGER_MS = 3000

export class SessionsBridge {
  private readonly kaminToTab = new Map<string, string>()
  private readonly tabToKamin = new Map<string, string>()
  // Последние опубликованные значения — дедуп широковещательных апдейтов.
  private readonly lastStatus = new Map<string, string>()
  private readonly lastWorking = new Map<string, boolean>()
  // Tabs whose working-state is now driven by the CLI's lifecycle hooks; once a
  // tab is in here the flappy OSC fallback is ignored for its bridgeWorking dot.
  private readonly hookDrivenTabs = new Set<string>()
  private readonly subs: kaminide.Disposable[] = []

  constructor(private readonly host: BridgeHost, private readonly openChat: () => void) {}

  install(): void {
    // Bind Claude conversationId + AI title back onto the owning KaminIDE session.
    const prevConv = ConnectionManager.onConversationId
    ConnectionManager.onConversationId = (tabId, conversationId) => {
      prevConv?.(tabId, conversationId)
      const kid = this.tabToKamin.get(tabId)
      if (!kid) return
      const cwd = this.host.getTabManager().getTab(tabId)?.config.cwd ?? null
      const meta: BridgeMeta = { conversationId, cwd }
      void kaminide.sessions.updateSession(kid, { metadata: { bridge: meta } })
    }
    const prevTitle = ConnectionManager.onSessionTitle
    ConnectionManager.onSessionTitle = (tabId, title) => {
      prevTitle?.(tabId, title)
      const kid = this.tabToKamin.get(tabId)
      if (kid && title) void kaminide.sessions.updateSession(kid, { name: title })
    }

    // Mirror connection status + working state onto the owning KaminIDE session's
    // metadata → the native session row shows a coloured/pulsing status dot.
    // (Shallow-merged keys, so each preserves the other + the bound conversation.)
    ConnectionManager.onStatus = (tabId, status) => {
      const kid = this.tabToKamin.get(tabId)
      if (!kid) return
      // Только СМЕНА значения: каждый updateSession рассылает всем клиентам
      // полный снапшот сессий, а статус повторяется на каждом чихе сокета —
      // с тёплым пулом (6 живых сессий) это был постоянный широковещательный шум.
      if (this.lastStatus.get(tabId) === status) return
      this.lastStatus.set(tabId, status)
      void kaminide.sessions.updateSession(kid, { metadata: { bridgeStatus: status } })
    }
    ConnectionManager.onActivity = (tabId, working, hookDriven) => {
      // Deterministic hooks (UserPromptSubmit→working, Stop→idle) beat the OSC
      // spinner heuristic: once a tab has emitted a hook event, drop its flappy
      // OSC fallback so the native session-row dot doesn't blink off mid-turn.
      if (hookDriven) this.hookDrivenTabs.add(tabId)
      else if (this.hookDrivenTabs.has(tabId)) return
      const kid = this.tabToKamin.get(tabId)
      if (!kid) return
      if (this.lastWorking.get(tabId) === working) return // см. onStatus
      this.lastWorking.set(tabId, working)
      void kaminide.sessions.updateSession(kid, { metadata: { bridgeWorking: working } })
    }

    // Activate the owning KaminIDE session for a tab. Two callers: the
    // "session finished" toast's Open action, and a switch made in the
    // webview's own tab strip — which otherwise moved the Bridge tab while the
    // host's session list and titlebar stayed highlighting the session the user
    // had just left.
    setSessionActivateHandler((tabId) => {
      const kid = this.tabToKamin.get(tabId)
      if (kid) kaminide.sessions.setActiveSession(kid)
    })

    // Publish whether the chat has caught up with the active session, so host
    // surfaces can show a switch in progress instead of silently claiming the
    // new session while the old transcript is still on screen. Fires from both
    // ends of the gap: the switch that opens it, and the report that closes it.
    this.subs.push({ dispose: onDisplayedTabChange(() => this.publishDisplayGap()) })

    this.subs.push(kaminide.sessions.onDidChangeActiveSession((s) => { this.onActive(s); this.publishDisplayGap() }))
    // Free the Bridge tab when its owning KaminIDE session is deactivated
    // ("Disconnect — free from memory") or deleted. Without this the tab's
    // WebSocket to the server + the server-side Claude PTY stay fully alive
    // until the reaper's idle timeout (~30min), so "free from memory" wouldn't
    // actually free anything. onDidChangeActiveSession never reports this — the
    // deactivated session simply stops being active — so we watch the full list.
    this.subs.push(kaminide.sessions.onDidChangeSessions((e) => this.onSessionsChanged(e)))
    // The active-session event may not fire for the already-active session at
    // boot — bind it explicitly. Re-bind once a token is configured (the gate
    // lives in the chat panel, so the first activation before config is a no-op).
    ipcMain.on("set-config", () => {
      if (this.host.getConfigStore().get().token) {
        this.onActive(kaminide.sessions.active)
        this.scheduleWarmRestore()
      }
    })
    this.onActive(kaminide.sessions.active)
    this.scheduleWarmRestore()
  }

  // ─── Warm pool ────────────────────────────────────────────────────────────
  // Sessions live continuously on the extension backend: every OPEN session —
  // not just the active one — gets its tab (WS + server session + transcript
  // cache) restored shortly after boot, in the background. Switching to one is
  // then a pointer flip the webview renders from its per-session store in
  // milliseconds, instead of a cold connect+replay with a loading screen.
  private warmTimer: ReturnType<typeof setTimeout> | null = null

  private scheduleWarmRestore(): void {
    if (this.warmTimer) return
    this.warmTimer = setTimeout(() => {
      this.warmTimer = null
      try { this.warmRestore() } catch { /* warm pool is an optimisation — never break boot */ }
    }, WARM_RESTORE_DELAY_MS)
  }

  private warmRestore(): void {
    const cfg = this.host.getConfigStore().get()
    if (!cfg.token) return
    const tm = this.host.getTabManager()
    const activeId = kaminide.sessions.active?.id
    const candidates = kaminide.sessions.all
      .filter((s) => s.open && s.id !== activeId)
      // Already warm (live tab) → nothing to do.
      .filter((s) => { const t = this.kaminToTab.get(s.id); return !(t && tm.getTab(t)) })
      // Only sessions with a bound conversation can be resumed.
      .filter((s) => !!((s.metadata?.bridge as BridgeMeta | undefined)?.conversationId))
      .sort((a, b) => b.lastOpened - a.lastOpened)
      .slice(0, WARM_POOL_MAX)
    // Создание фоновых табов дёшево (без connect) — все сразу.
    for (const s of candidates) {
      const meta = s.metadata?.bridge as BridgeMeta
      const tabId = tm.resumeSession({
        serverUrl: cfg.serverUrl,
        token: cfg.token,
        cwd: this.projectFolder(s),
        conversationId: meta.conversationId!,
        sessionLabel: s.name,
      }, { background: true })
      if (tabId) {
        this.kaminToTab.set(s.id, tabId)
        this.tabToKamin.set(tabId, s.id)
      }
    }
    // Staggered webview pre-fill from the disk mirror (see PREFILL_*). If the
    // user switched to a tab meanwhile, the replay dedups in the store.
    candidates.forEach((s, i) => {
      const t = setTimeout(() => {
        this.prefillTimers.delete(t)
        const tabId = this.kaminToTab.get(s.id)
        const conn = tabId ? this.host.getTabManager().getConnection(tabId) : undefined
        if (!conn) return
        void conn.replayFromMirror().then(() => { conn.replayJsonlToRenderer() })
          .catch(() => { conn.replayJsonlToRenderer() })
      }, PREFILL_START_MS + i * PREFILL_STAGGER_MS)
      this.prefillTimers.add(t)
    })
  }

  private readonly prefillTimers = new Set<ReturnType<typeof setTimeout>>()

  /** Ask the CLI to regenerate this session's title from the chat (old-Bridge
   *  "Auto-rename"). Clears the user's name-sticky flag first so the AI title
   *  the CLI emits next is allowed to land (see host updateSession guard), then
   *  injects `/rename` into the live PTY. No-op if the session has no live tab. */
  private regen(tabId: string, kaminId: string | undefined): void {
    const tm = this.host.getTabManager()
    if (!tm.getTab(tabId)) return
    if (kaminId) void kaminide.sessions.updateSession(kaminId, { metadata: { nameSetByUser: false } })
    tm.submitText(tabId, '/rename')
  }
  /** From the native session context menu (host renderer → command). */
  regenerateTitleByKamin(kaminId: string): void {
    const tabId = this.kaminToTab.get(kaminId)
    if (tabId) this.regen(tabId, kaminId)
  }
  /** From the chat-panel header button (webview → send channel). */
  regenerateTitleByTab(tabId: string): void {
    this.regen(tabId, this.tabToKamin.get(tabId))
  }

  /** Re-assert the active session's tab + re-push its cached JSONL. Per VS Code a
   *  hidden webview view drops postMessage (its iframe stays mounted but the host
   *  suspends delivery), so a session switch or live entries behind a collapsed
   *  panel are missed. Runs only when the revealed view actually has a recorded
   *  gap — BridgeHost tracks that per view at the dropped send itself. Broadcasts
   *  (so every currently-visible panel is caught up at once); the webview dedups
   *  on `_ord`, so a resync never duplicates or reorders entries. */
  resyncActive(): void {
    // Коалесценция: attach-волна на старте (все aux-вью привязываются подряд,
    // каждый помечен stale) дёргала бы полный реплей на каждый reveal.
    if (this.resyncTimer) return
    this.resyncTimer = setTimeout(() => {
      this.resyncTimer = null
      const tm = this.host.getTabManager()
      const active = tm.getActiveTabId()
      if (!active) return
      tm.switchTab(active) // re-send tab:switched so panels bind the right session
      tm.getActiveConnection()?.replayJsonlToRenderer() // re-push cached JSONL missed while hidden
    }, 300)
  }
  private resyncTimer: ReturnType<typeof setTimeout> | null = null

  /** Drop the kamin↔tab mapping (+ hook-driven flag) for a tab we just tore
   *  down. Reactivation goes back through `onActive`, which — finding no live
   *  tab — resumes the conversation fresh from the session's stored metadata. */
  private forgetTab(tabId: string): void {
    const kid = this.tabToKamin.get(tabId)
    this.tabToKamin.delete(tabId)
    if (kid !== undefined) this.kaminToTab.delete(kid)
    this.hookDrivenTabs.delete(tabId)
  }

  /** React to session list changes: a session turning `open:false` = the user
   *  disconnected it → free its Bridge tab (server PTY + WS); a removed session
   *  = deleted → close its tab entirely. `changed` also fires for our own
   *  metadata writes (status/working/title), which keep `open` truthy, so those
   *  are ignored — we act only on an explicit `open === false`. */
  private onSessionsChanged(e: kaminide.SessionsChangeEvent): void {
    const tm = this.host.getTabManager()
    for (const s of e.removed) {
      const tabId = this.kaminToTab.get(s.id)
      if (tabId && tm.getTab(tabId)) { tm.closeTab(tabId); this.forgetTab(tabId) }
    }
    for (const s of e.changed) {
      if (s.open !== false) continue // only the deactivate transition frees the tab
      const tabId = this.kaminToTab.get(s.id)
      // closeTab, not disconnectTab: forgetTab drops the mapping, so a mere
      // disconnect left a zombie tab (with its full jsonl cache in memory)
      // that nothing could ever reach again — and reactivation spawned a
      // SECOND tab for the same session (audit #70 C6). closeTab persists
      // the session to "inactive" first (onTabClosing), so nothing is lost.
      if (tabId && tm.getTab(tabId)) { tm.closeTab(tabId); this.forgetTab(tabId) }
    }
  }

  private projectFolder(s: KaminSession): string | undefined {
    return kaminide.sessions.projects.find((p) => p.id === s.projectId)?.folderPath ?? undefined
  }

  /** Mark the ACTIVE session as showing-or-not. Only the active one can be in
   *  the gap: every other session's chat is not on screen at all, so a stale
   *  flag left on it would render as a permanent spinner nothing would clear. */
  private publishDisplayGap(): void {
    const tm = this.host.getTabManager()
    const activeTab = tm.getActiveTabId()
    if (!activeTab) return
    const kid = this.tabToKamin.get(activeTab)
    if (!kid) return
    const showing = getDisplayedTab() === activeTab
    if (this.lastPublishedGap === `${kid}:${String(showing)}`) return
    this.lastPublishedGap = `${kid}:${String(showing)}`
    void kaminide.sessions.updateSession(kid, { metadata: { bridgeShowing: showing } })
  }

  private lastPublishedGap = ''

  private onActive(s: KaminSession | undefined): void {
    if (!s) return
    const tm = this.host.getTabManager()
    const existing = this.kaminToTab.get(s.id)
    if (existing && tm.getTab(existing)) {
      tm.switchTab(existing)
      this.openChat()
      return
    }
    const cfg = this.host.getConfigStore().get()
    if (!cfg.token) { this.openChat(); return }

    const cwd = this.projectFolder(s)
    const meta = (s.metadata?.bridge ?? null) as BridgeMeta | null
    const tabId = meta?.conversationId
      ? tm.resumeSession({ serverUrl: cfg.serverUrl, token: cfg.token, cwd, conversationId: meta.conversationId, sessionLabel: s.name })
      : tm.createTab({ serverUrl: cfg.serverUrl, token: cfg.token, cwd })
    if (tabId) {
      this.kaminToTab.set(s.id, tabId)
      this.tabToKamin.set(tabId, s.id)
    }
    this.openChat()
  }

  dispose(): void {
    if (this.warmTimer) { clearTimeout(this.warmTimer); this.warmTimer = null }
    for (const t of this.prefillTimers) clearTimeout(t)
    this.prefillTimers.clear()
    for (const d of this.subs) d.dispose()
  }
}
