import { useEffect } from 'preact/hooks'

import type { KaminBridgeApi } from '../../shared/types'

// Signals
import { tabs, applyTabSwitch } from '../signals/tabs'
import { sidebarMode, activeCustomizePanel, installedSkills, hasToken, filePanelVisible, filePanelBottomVisible, filePanelSplit, initializing } from '../signals/ui'
import { applyRestoredState } from '../lib/tabs-persist'
import type { WebviewRole } from './useBridgeListeners'
import { setFrameRetentionProvider } from '../lib/host-ready'
import { jsonlEntriesByTab } from '../signals/jsonl'

/**
 * Runs the one-time initialization logic on mount:
 * - Fetches existing tabs from the bridge
 * - Sets the active tab
 * - Requests tree update and JSONL replay
 * - Falls back to customize/settings only if no config exists (first launch)
 */
export function useInit(bridge: KaminBridgeApi, role: WebviewRole = 'chat'): void {
  useEffect(() => {
    // Name what THIS panel retains in freeze/OOM reports. The shared heap number
    // is identical from every panel (one same-origin renderer), so counts are the
    // only attributable signal — see FrameRetention in lib/host-ready.
    setFrameRetentionProvider(() => {
      let entries = 0
      const store = jsonlEntriesByTab.value
      for (const list of store.values()) entries += list.length
      return { role, tabs: store.size, entries }
    })
  }, [role])

  useEffect(() => {
    // Load installed skills so the slash-command autocomplete can surface them.
    // CLI's `findCommand` (src/commands.ts) accepts either the canonical
    // `<pluginName>:<cmdSlug>` form (its `cmd.name`) or `userFacingName()`
    // which is `frontmatter.name ?? cmd.name`. We mirror the CLI's own
    // typeahead: when the plugin author supplied `name:` in frontmatter, use
    // that short form (matches what the CLI prints); otherwise use the
    // canonical `<pluginName>:<slug>` form. No `/plugin:` prefix — CLI has
    // no such prefix and would reject it.
    //
    // Collision handling — mirrors CLI semantics (formatDescriptionWithSource
    // in src/commands.ts:728): when two plugins both ship a command with the
    // same frontmatter `name`, both rows stay in the list (CLI intentionally
    // does not deduplicate — see commentary in commandSuggestions.ts:476) and
    // the description is prefixed with `(pluginName)` to disambiguate. On top
    // of that we do one thing CLI can't: for the SECOND+ colliding row we
    // switch the inserted slash text to the canonical `<plugin>:<slug>` form,
    // which `findCommand` also accepts — so picking the "right" row routes to
    // the right plugin instead of silently hitting whichever one CLI loaded
    // first.
    bridge.listSkills().then(list => {
      interface Raw { name: string; source: string; description: string; displayName?: string; argumentHint?: string }
      const raws: Raw[] = list.map((s: any) => ({
        name: String(s.name),
        source: typeof s.source === 'string' ? s.source : 'user',
        description: s.description || '',
        displayName: typeof s.displayName === 'string' ? s.displayName : undefined,
        argumentHint: typeof s.argumentHint === 'string' ? s.argumentHint : undefined,
      }))

      function canonicalName(r: Raw): string | null {
        if (!r.source.startsWith('plugin:')) return null
        const pluginName = r.source.slice('plugin:'.length)
        const slug = r.name.replace(/\//g, ':')
        return `${pluginName}:${slug}`
      }
      function shortName(r: Raw): string | null {
        if (!r.source.startsWith('plugin:')) return r.displayName ?? r.name
        return r.displayName ?? null
      }

      // Tally displayName collisions among plugin rows so we can fall back to
      // the canonical form for the duplicates.
      const shortCount = new Map<string, number>()
      for (const r of raws) {
        const sh = shortName(r)
        if (sh) shortCount.set(sh, (shortCount.get(sh) ?? 0) + 1)
      }

      installedSkills.value = raws.map(r => {
        const isPlugin = r.source.startsWith('plugin:')
        const pluginName = isPlugin ? r.source.slice('plugin:'.length) : ''
        const sh = shortName(r)
        const canon = canonicalName(r)
        let slashName: string
        if (isPlugin) {
          // Collision → use canonical `/<plugin>:<slug>` for unambiguous
          // routing. Otherwise prefer the short frontmatter name, falling
          // back to canonical if the plugin didn't supply one.
          slashName = sh && (shortCount.get(sh) ?? 0) > 1
            ? `/${canon}`
            : `/${sh ?? canon}`
        } else {
          slashName = `/${sh}`
        }
        let description = isPlugin
          ? `(${pluginName}) ${r.description || 'Plugin skill'}`
          : (r.description || 'User skill')
        if (r.argumentHint) description += ` (${r.argumentHint})`
        return { name: slashName, description }
      })
    }).catch(() => { /* skills dir may not exist yet */ })

    // Hydrate persisted file-panel visibility + split. The width itself
    // is hydrated inside FilePanel.tsx so it can compute the intended
    // ratio against the live viewport at the same moment, instead of
    // racing the panel's own mount.
    bridge.getLayout().then((l) => {
      if (typeof l?.filePanelVisible === 'boolean') filePanelVisible.value = l.filePanelVisible
      if (typeof l?.filePanelBottomVisible === 'boolean') filePanelBottomVisible.value = l.filePanelBottomVisible
      if (typeof l?.filePanelSplit === 'number' && l.filePanelSplit > 0 && l.filePanelSplit < 1) filePanelSplit.value = l.filePanelSplit
    }).catch(() => { /* first run: defaults stand */ })

    async function init(): Promise<void> {
      // Seed the gate signal first so NoTokenGateModal can decide to render
      // before any tab/jsonl work. If there's no token, the gate appears
      // immediately on top of everything; the rest of init still runs so
      // saved tabs are ready to display once the user unblocks.
      const config = await bridge.getConfig()
      hasToken.value = !!(config.serverUrl && config.token)

      const existingTabs = await bridge.listTabs()
      if (existingTabs.length > 0) {
        tabs.value = existingTabs
      }
      // Always restore persisted state, even when there are no live tabs —
      // a user who restarts the app with all tabs closed still needs their
      // pinned chips (as ghosts) so they can click to revive sessions.
      // Token mismatch returns [] which naturally resets pinned state.
      try {
        const persisted = await bridge.restoreTabsState()
        if (Array.isArray(persisted) && persisted.length > 0) {
          applyRestoredState(persisted, existingTabs)
        }
      } catch { /* restore is best-effort */ }
      if (existingTabs.length > 0) {
        const activeId = await bridge.getActiveTab()
        // Через seq-guard (seq=0 проигрывает ЛЮБОМУ реальному switch): между
        // await'ами init мог прилететь tab:switched, и прямая запись затирала
        // его stale-значением — кадр «не тот чат» после reload (аудит #70 C8).
        applyTabSwitch(activeId || existingTabs[0].id, 0)
        bridge.requestTreeUpdate()
        // The customize sections render none of the conversation and the host
        // withholds jsonl from them — a replay they can't receive is a full
        // server-side transcript re-read per section (×10) thrown away. Only the
        // panels that consume the stream ask for it.
        if (role !== 'customize') {
          try { bridge.requestJsonlReplay() } catch { /* preload not yet rebuilt */ }
        }
      }
      // First launch with no token: the NoTokenGateModal covers the whole UI,
      // so we leave navigation on the default sessions view — after the user
      // creates a token the gate drops and they land in the chat immediately.
    }
    init().catch(console.error).finally(() => { initializing.value = false })
  }, [])
}
