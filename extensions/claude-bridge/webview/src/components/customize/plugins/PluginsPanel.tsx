import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import type { MarketplaceInfo, MarketplacePlugin } from '../../../../shared/types'
import { PluginsTabs } from './PluginsTabs'
import { MarketplaceRow } from './MarketplaceRow'
import { MarketplaceCloneForm } from './MarketplaceCloneForm'
import { PluginsGrid } from './PluginsGrid'
import { PluginCard } from './PluginCard'
import { showToast } from '../../../signals/toasts'
import { readPluginsCache, writePluginsCache } from './plugins-cache'
import styles from './PluginsPanel.module.css'

type Tab = 'active' | 'anthropic' | 'personal'

function normalizeForSearch(str: string): string {
  return str.replace(/[-_]/g, ' ').toLowerCase()
}

function pluginMatchesSearch(p: { name: string; description?: string }, q: string): boolean {
  if (!q) return true
  const nq = normalizeForSearch(q)
  return normalizeForSearch(p.name).includes(nq) || normalizeForSearch(p.description || '').includes(nq)
}

export function PluginsPanel(): JSX.Element {
  const bridge = useBridge()
  const [tab, setTab] = useState<Tab>('active')
  const [search, setSearch] = useState('')
  const [marketplaces, setMarketplaces] = useState<MarketplaceInfo[]>([])
  const [selectedMarketplace, setSelectedMarketplace] = useState<string | null>(null)
  const [plugins, setPlugins] = useState<MarketplacePlugin[]>([])
  const [installedPlugins, setInstalledPlugins] = useState<any[]>([])
  const [loading, setLoading] = useState(false)
  // Background revalidation in progress (cache already painted) — drives the
  // thin non-blocking progress bar instead of the full-surface spinner.
  const [bgRefreshing, setBgRefreshing] = useState(false)
  const [loadProgress, setLoadProgress] = useState<{ done: number; total: number; current: string | null } | null>(null)
  const [showCloneForm, setShowCloneForm] = useState(false)

  // Subscribe once to the browse-progress stream. The main process emits
  // these before every git-clone inside `plugins:browse-marketplace`, which
  // is why a cold fetch of `claude-plugins-official` stalls for 30+s on a
  // bare spinner. Now we surface "Loading N/TOTAL — <pluginName>" instead.
  useEffect(() => {
    const off = bridge.onPluginsBrowseProgress((p) => {
      if (p.done >= p.total) setLoadProgress(null)
      else setLoadProgress({ done: p.done, total: p.total, current: p.current })
    })
    return off
  }, [bridge])

  async function loadMarketplaces(activeTab: Tab, force = false): Promise<void> {
    if (activeTab === 'active') {
      await loadInstalled(force)
      return
    }
    try {
      const all = await bridge.listMarketplaces()
      const filtered = all.filter((m) =>
        activeTab === 'anthropic' ? m.isAnthropicOfficial : !m.isAnthropicOfficial
      )
      setMarketplaces(filtered)
      if (filtered.length > 0) {
        const first = selectedMarketplace && filtered.find((m) => m.name === selectedMarketplace)
          ? selectedMarketplace
          : filtered[0].name
        setSelectedMarketplace(first)
        await loadMarketplacePlugins(first, force)
      } else {
        setPlugins([])
      }
    } catch {
      setPlugins([])
    }
  }

  async function loadInstalled(force = false): Promise<void> {
    const cached = force ? null : readPluginsCache<any[]>('installed')
    if (cached) {
      setInstalledPlugins(cached); setLoading(false); setBgRefreshing(true)
    } else {
      setLoading(true)
    }
    try {
      const result = await bridge.listInstalledPlugins()
      setInstalledPlugins(result ?? [])
      writePluginsCache('installed', result ?? [])
    } catch {
      if (!cached) setInstalledPlugins([])
    }
    setLoading(false); setBgRefreshing(false); setLoadProgress(null)
  }

  // Cache-first: paint the last-known list INSTANTLY from persisted state, then
  // revalidate in the BACKGROUND (thin progress bar, panel stays interactive).
  // `force` (the refresh button) bypasses the cache for a full reload.
  async function loadMarketplacePlugins(name: string, force = false): Promise<void> {
    const cacheKey = `mkt.${name}`
    const cached = force ? null : readPluginsCache<MarketplacePlugin[]>(cacheKey)
    if (cached) {
      setPlugins(cached); setLoading(false); setBgRefreshing(true)
    } else {
      setLoading(true)
    }
    try {
      const result = await bridge.browseMarketplace(name)
      setPlugins(result ?? [])
      writePluginsCache(cacheKey, result ?? [])
    } catch {
      if (!cached) setPlugins([])
    }
    setLoading(false); setBgRefreshing(false); setLoadProgress(null)
  }

  useEffect(() => {
    loadMarketplaces(tab)
  }, [tab])

  function handleTabChange(newTab: Tab): void {
    setTab(newTab)
    setSelectedMarketplace(null)
  }

  async function handleMarketplaceSelect(name: string): Promise<void> {
    setSelectedMarketplace(name)
    await loadMarketplacePlugins(name)
  }

  async function handleRefresh(): Promise<void> {
    await loadMarketplaces(tab, true) // force: bypass cache for a full reload
  }

  async function handleCloneDone(name: string): Promise<void> {
    setShowCloneForm(false)
    setSelectedMarketplace(name)
    await loadMarketplaces(tab)
  }

  async function handleOpenAsMarketplace(mktName: string): Promise<void> {
    try {
      const all = await bridge.listMarketplaces()
      const target = all.find((m) => m.name === mktName)
      if (!target) return
      const nextTab: Tab = target.isAnthropicOfficial ? 'anthropic' : 'personal'
      setSelectedMarketplace(mktName)
      if (nextTab !== tab) {
        setTab(nextTab)
      } else {
        await handleMarketplaceSelect(mktName)
      }
    } catch {}
  }

  async function handleLocalPlugin(): Promise<void> {
    try {
      const result = await bridge.installLocalPlugin()
      if (result) await handleRefresh()
    } catch (err: any) {
      showToast({ type: 'error', title: 'Plugin install failed', message: err?.message || String(err), duration: 5000 })
    }
  }

  const isActiveTab = tab === 'active'
  const showMarketplaceRow = !isActiveTab

  let content: JSX.Element
  if (loading) {
    content = loadProgress ? (
      <div class={styles.loaderRow}>
        <div class={styles.loaderInner}>
          <i class="fas fa-spinner fa-spin" />
          <span class={styles.loaderLabel}>Loading</span>
          <span class={styles.loaderCounter}>{loadProgress.done + 1}/{loadProgress.total}</span>
          <span class={styles.loaderCurrent}>{loadProgress.current || ''}</span>
        </div>
      </div>
    ) : (
      <div class={styles.placeholder}>
        <i class="fas fa-spinner fa-spin" /> Loading...
      </div>
    )
  } else if (isActiveTab) {
    const filtered = installedPlugins.filter((p) => pluginMatchesSearch(p, search))
    if (filtered.length === 0) {
      content = <div class={styles.placeholder}>No active plugins installed</div>
    } else {
      content = (
        <>
          {filtered.map((p) => (
            <PluginCard
              key={`${p.marketplace}/${p.name}`}
              name={p.name}
              version={p.version}
              description={(p as any).description}
              marketplace={p.marketplace}
              isCached={(p as any).isCached}
              isInstalled={true}
              enabled={(p as any).enabled !== false}
              installPath={p.installPath}
              counts={(p as any).counts}
              dependencies={(p as any).dependencies}
              isRemoteSource={(p as any).isRemoteSource}
              onRefresh={loadInstalled}
              onOpenAsMarketplace={handleOpenAsMarketplace}
            />
          ))}
        </>
      )
    }
  } else {
    const filtered = plugins.filter((p) => pluginMatchesSearch(p, search))
    if (filtered.length === 0) {
      content = <div class={styles.placeholder}>No plugins found</div>
    } else {
      content = (
        <>
          {filtered.map((p) => (
            <PluginCard
              key={`${p.marketplace}/${p.name}`}
              name={p.name}
              version={p.version}
              description={p.description}
              marketplace={p.marketplace}
              author={(p as any).author}
              isCached={(p as any).isCached}
              isInstalled={p.isInstalled}
              installPath={(p as any).installPath}
              pluginPath={p.pluginPath}
              counts={(p as any).counts}
              isRemoteSource={(p as any).isRemoteSource}
              cloneFailed={(p as any).cloneFailed}
              cloneError={(p as any).cloneError}
              cloneUrl={(p as any).cloneUrl}
              cloneRef={(p as any).cloneRef}
              isLocalSourceMissing={(p as any).isLocalSourceMissing}
              declaredSourcePath={(p as any).declaredSourcePath}
              onRefresh={() => loadMarketplacePlugins(selectedMarketplace!)}
            />
          ))}
        </>
      )
    }
  }

  return (
    <div class={styles.panel}>
      <div class={styles.topSection}>
        <h2 class={styles.title}>Browse plugins</h2>
        <p class={styles.subtitle}>Extend how Claude performs tasks with ready-to-use workflows.</p>
        <PluginsTabs
          activeTab={tab}
          onTabChange={handleTabChange}
          searchValue={search}
          onSearchChange={setSearch}
        />
      </div>

      {showMarketplaceRow && (
        <MarketplaceRow
          marketplaces={marketplaces}
          selectedName={selectedMarketplace}
          onSelect={handleMarketplaceSelect}
          onRefresh={handleRefresh}
          onAddGithubOrUrl={() => setShowCloneForm(true)}
          onAddLocal={handleLocalPlugin}
          allowAdd={tab === 'personal'}
        />
      )}

      {showCloneForm && (
        <MarketplaceCloneForm
          onDone={handleCloneDone}
          onCancel={() => setShowCloneForm(false)}
        />
      )}

      {bgRefreshing && (
        <div class={styles.bgBar} role="status" aria-live="polite">
          <div
            class={styles.bgBarFill}
            style={{ width: loadProgress && loadProgress.total > 0 ? `${Math.round((loadProgress.done / loadProgress.total) * 100)}%` : '100%' }}
            data-indeterminate={loadProgress ? undefined : 'true'}
          />
          <span class={styles.bgBarLabel}>
            <i class="fas fa-rotate fa-spin" /> Refreshing{loadProgress ? ` ${loadProgress.done + 1}/${loadProgress.total}` : '…'}
            {loadProgress?.current ? ` · ${loadProgress.current}` : ''}
          </span>
        </div>
      )}

      <PluginsGrid>
        {content}
      </PluginsGrid>
    </div>
  )
}
