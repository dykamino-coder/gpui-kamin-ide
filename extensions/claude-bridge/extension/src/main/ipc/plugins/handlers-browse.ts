// IPC handlers: marketplace plugin browsing (top-level + nested).
// Extracted from `electron/main/ipc/plugins.ts` (Sprint 2 / Stage C, C2).

import { ipcMain, type IpcMainInvokeEvent } from '@kaminide/host-compat'
import path from 'path'
import fs from 'fs'
import os from 'os'
import { resolvePluginSource } from './shared'
import { countPluginContents, countPluginLspServers } from './content-scan'

export function registerBrowseHandlers(): void {
  ipcMain.handle('plugins:browse-marketplace', async (event: IpcMainInvokeEvent, marketplaceName: string) => {
    const knownFile = path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json')
    try {
      if (!fs.existsSync(knownFile)) return []
      const known = JSON.parse(fs.readFileSync(knownFile, 'utf-8'))
      const marketplace = known[marketplaceName]
      if (!marketplace?.installLocation) return []

      const baseDir = marketplace.installLocation

      let installedKeys = new Set<string>()
      // Parse installed_plugins.json ONCE (was re-read + re-parsed per plugin
      // inside the loop for the isCached check — a real N+1 on a 400-plugin
      // marketplace). Keep the plugins map to resolve installPath below.
      let installedPluginsMap: Record<string, Array<{ installPath?: string }>> = {}
      const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
      if (fs.existsSync(installedFile)) {
        try {
          const instData = JSON.parse(fs.readFileSync(installedFile, 'utf-8'))
          if (instData.version === 2 && instData.plugins) {
            installedKeys = new Set(Object.keys(instData.plugins))
            installedPluginsMap = instData.plugins
          }
        } catch {}
      }

      const results: any[] = []

      const marketplaceJsonPath = path.join(baseDir, '.claude-plugin', 'marketplace.json')
      if (fs.existsSync(marketplaceJsonPath)) {
        try {
          const mktData = JSON.parse(fs.readFileSync(marketplaceJsonPath, 'utf-8'))
          if (Array.isArray(mktData.plugins)) {
            // Emit progress events so the renderer can show "Resolving
            // plugin N of TOTAL — name" instead of a bare "Loading...".
            // First resolvePluginSource on a cold marketplace triggers
            // real git clones (tens of MB per plugin), which is why this
            // loop legitimately takes tens of seconds.
            // Dedupe declared plugins by name BEFORE dispatch. Two entries
            // with the same name resolve to the SAME
            // `<baseDir>/plugins/<name>/` clone dir, so concurrent workers
            // would both see "not cloned yet" and race two git clones into
            // one directory. Keep the first occurrence — matches the old
            // sequential behaviour (later dupes hit the "already cloned —
            // reuse" fast path). Makes the post-loop dedupe redundant but we
            // keep that as a belt-and-suspenders guard.
            const seenNames = new Set<string>()
            const uniquePlugins: any[] = mktData.plugins.filter((pl: any) => {
              const n = pl?.name || 'unknown'
              if (seenNames.has(n)) return false
              seenNames.add(n)
              return true
            })
            const total = uniquePlugins.length
            const progressChannel = 'plugins:browse-progress'
            event.sender.send(progressChannel, { marketplace: marketplaceName, done: 0, total, current: null })

            // Resolve one plugin → its result row. Extracted so the driver
            // below can run several concurrently. resolvePluginSource handles
            // directory / url / git / github shapes; for remote URLs it
            // shallow-clones into `<baseDir>/plugins/<name>/` on first visit
            // with auth propagated from the parent marketplace URL.
            const buildPluginRow = async (p: any, pluginName: string): Promise<any> => {
              const resolved = await resolvePluginSource(p, marketplaceName, baseDir)
              if (!resolved.dir) {
                // Failed to resolve — distinguish between remote clone
                // failure (retryable: network/auth) and local-source-
                // missing (not retryable: the marketplace declared a
                // relative path that just isn't shipped in this clone —
                // e.g. `./external_plugins/autofix-bot` that never got
                // checked out, or a submodule path that wasn't inited).
                const srcTag = typeof p?.source === 'object' ? p.source?.source : 'legacy'
                const isRemoteErr = srcTag === 'url' || srcTag === 'git' || srcTag === 'github'
                const isLocalStringSource = typeof p?.source === 'string'
                const isLocalSourceMissing = isLocalStringSource && !isRemoteErr
                return {
                  name: pluginName,
                  version: p.version || '',
                  description: p.description || '',
                  author: p.author?.name || (typeof p.author === 'string' ? p.author : '') || mktData.owner?.name || '',
                  marketplace: marketplaceName,
                  keywords: [],
                  pluginPath: '',
                  isInstalled: false,
                  isCached: false,
                  hasCommands: false,
                  counts: { commands: 0, agents: 0, skills: 0, lspServers: 0 },
                  isRemoteSource: isRemoteErr,
                  cloneFailed: true,
                  cloneError: resolved.error || 'unknown error',
                  cloneUrl: resolved.url,
                  cloneRef: resolved.ref,
                  isLocalSourceMissing,
                  declaredSourcePath: isLocalStringSource ? (p.source as string) : undefined,
                }
              }
              const pluginDir = resolved.dir
              const hasCommands = fs.existsSync(path.join(pluginDir, 'commands'))
              const key = `${pluginName}@${marketplaceName}`

              let author = ''
              let keywords: string[] = []
              const pluginJsonPath = path.join(pluginDir, '.claude-plugin', 'plugin.json')
              if (fs.existsSync(pluginJsonPath)) {
                try {
                  const pj = JSON.parse(fs.readFileSync(pluginJsonPath, 'utf-8'))
                  author = pj.author?.name || (typeof pj.author === 'string' ? pj.author : '')
                  keywords = Array.isArray(pj.keywords) ? pj.keywords : []
                } catch {}
              }

              if (!author && mktData.owner?.name) author = mktData.owner.name
              if (!author && typeof p.author === 'string') author = p.author
              if (!author && p.author?.name) author = p.author.name

              if (keywords.length === 0 && Array.isArray(p.keywords)) keywords = p.keywords
              if (p.category && !keywords.includes(p.category)) keywords = [p.category, ...keywords]

              const counts = countPluginContents(pluginDir, baseDir)
              counts.lspServers = countPluginLspServers(pluginName, marketplaceName, pluginDir)
              // When a marketplace monorepo plugin declares its content via
              // explicit arrays (e.g. `skills: ["./skills/xlsx", ...]`),
              // prefer those counts over the directory scan — otherwise we
              // over-count (whole skills/ dir) or under-count (skills live
              // elsewhere). Matches how CLI interprets these manifests.
              if (Array.isArray(p.skills)) counts.skills = p.skills.length
              if (Array.isArray(p.commands)) counts.commands = p.commands.length
              if (Array.isArray(p.agents)) counts.agents = p.agents.length
              let isCached = false
              if (installedKeys.has(key)) {
                const instEntry = installedPluginsMap[key]?.[0]
                if (instEntry?.installPath && fs.existsSync(instEntry.installPath)) isCached = true
              }
              const srcTag = typeof p?.source === 'object' ? p.source?.source : undefined
              const isRemoteSource = srcTag === 'url' || srcTag === 'git' || srcTag === 'github'
              return {
                name: pluginName,
                version: p.version || '',
                description: p.description || '',
                author,
                marketplace: marketplaceName,
                keywords,
                pluginPath: pluginDir,
                isInstalled: installedKeys.has(key),
                isCached,
                hasCommands,
                counts,
                isRemoteSource,
              }
            }

            // Bounded-concurrency driver. A cold marketplace resolves each
            // remote plugin by a shallow git-clone into a UNIQUE per-plugin
            // dir, so different plugins can clone in parallel without racing;
            // running them one-at-a-time (the old sequential await) meant a
            // 400-plugin marketplace serialized 400 network round-trips.
            // Cap at CLONE_CONCURRENCY to avoid spawning hundreds of git
            // processes at once. Rows are written back by index to preserve
            // the marketplace's declared order.
            const CLONE_CONCURRENCY = 6
            const rows: any[] = new Array(total)
            let cursor = 0
            let done = 0
            const worker = async (): Promise<void> => {
              for (;;) {
                const idx = cursor++
                if (idx >= total) return
                const p = uniquePlugins[idx]
                const pluginName = p.name || 'unknown'
                event.sender.send(progressChannel, { marketplace: marketplaceName, done, total, current: pluginName })
                rows[idx] = await buildPluginRow(p, pluginName)
                done++
                event.sender.send(progressChannel, { marketplace: marketplaceName, done, total, current: pluginName })
              }
            }
            await Promise.all(
              Array.from({ length: Math.min(CLONE_CONCURRENCY, total || 1) }, () => worker()),
            )
            for (const row of rows) if (row) results.push(row)
            // Signal completion so the renderer can clear its progress badge.
            event.sender.send(progressChannel, { marketplace: marketplaceName, done: total, total, current: null })
            const seen = new Set<string>()
            const deduped = results.filter(p => {
              if (seen.has(p.name)) return false
              seen.add(p.name)
              return true
            })
            return deduped
          }
        } catch {}
      }

      let scanDir: string
      if (marketplaceName === 'anthropic-agent-skills') {
        scanDir = path.join(baseDir, 'skills')
      } else {
        scanDir = path.join(baseDir, 'plugins')
      }
      if (!fs.existsSync(scanDir)) return results

      function scanPluginDir(dir: string, pluginDirName: string) {
        const manifestPath = path.join(dir, '.claude-plugin', 'plugin.json')
        const commandsDir = path.join(dir, 'commands')
        const hasManifest = fs.existsSync(manifestPath)
        const hasCommands = fs.existsSync(commandsDir) && fs.statSync(commandsDir).isDirectory()

        if (!hasManifest && !hasCommands) return

        let name = pluginDirName
        let version = ''
        let description = ''
        let author = ''
        let keywords: string[] = []

        if (hasManifest) {
          try {
            const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf-8'))
            name = manifest.name || pluginDirName
            version = manifest.version || ''
            description = manifest.description || ''
            author = manifest.author?.name || (typeof manifest.author === 'string' ? manifest.author : '')
            keywords = Array.isArray(manifest.keywords) ? manifest.keywords : []
          } catch {}
        }

        const key = `${name}@${marketplaceName}`
        const counts = countPluginContents(dir, baseDir)
        counts.lspServers = countPluginLspServers(name, marketplaceName, dir)
        let isCached = false
        if (installedKeys.has(key)) {
          const ie = installedPluginsMap[key]?.[0]
          if (ie?.installPath && fs.existsSync(ie.installPath)) isCached = true
        }
        results.push({
          name,
          version,
          description,
          author,
          marketplace: marketplaceName,
          keywords,
          pluginPath: dir,
          isInstalled: installedKeys.has(key),
          isCached,
          hasCommands,
          counts,
        })
      }

      const entries = fs.readdirSync(scanDir, { withFileTypes: true }).filter(e => e.isDirectory())
      for (const entry of entries) {
        const entryPath = path.join(scanDir, entry.name)
        const hasManifest = fs.existsSync(path.join(entryPath, '.claude-plugin', 'plugin.json'))
        const hasCommands = fs.existsSync(path.join(entryPath, 'commands'))

        if (hasManifest || hasCommands) {
          scanPluginDir(entryPath, entry.name)
        } else {
          try {
            const subEntries = fs.readdirSync(entryPath, { withFileTypes: true }).filter(e => e.isDirectory())
            for (const sub of subEntries) {
              scanPluginDir(path.join(entryPath, sub.name), sub.name)
            }
          } catch {}
        }
      }

      return results
    } catch { return [] }
  })

  // Browse plugins DECLARED in a nested marketplace.json (the "broken
  // marketplace" case — a plugin whose installed directory actually carries
  // `.claude-plugin/marketplace.json` instead of `plugin.json`). Returns
  // one row per child plugin with the install status resolved against the
  // user's global `installed_plugins.json`. No git clones — declared plugin
  // sources need to be resolvable relative to the parent dir.
  ipcMain.handle('plugins:browse-nested-marketplace', (_event: IpcMainInvokeEvent, pluginPath: string) => {
    try {
      if (!pluginPath || !fs.existsSync(pluginPath)) return []
      const mktJson = path.join(pluginPath, '.claude-plugin', 'marketplace.json')
      if (!fs.existsSync(mktJson)) return []
      const mktData = JSON.parse(fs.readFileSync(mktJson, 'utf-8'))
      const nestedName: string = typeof mktData?.name === 'string' ? mktData.name : path.basename(pluginPath)
      const plugins: any[] = Array.isArray(mktData?.plugins) ? mktData.plugins : []
      const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
      let installedKeys = new Set<string>()
      if (fs.existsSync(installedFile)) {
        try {
          const inst = JSON.parse(fs.readFileSync(installedFile, 'utf-8'))
          if (inst?.version === 2 && inst?.plugins) installedKeys = new Set(Object.keys(inst.plugins))
        } catch {}
      }
      const results: any[] = []
      for (const p of plugins) {
        const name: string = p.name || 'unknown'
        // Resolve child directory. Supports the shapes we've observed in
        // the wild: `source: "./plugins/foo"` (relative dir), `source:
        // {source:'directory',path:'./foo'}`, or no source at all → look
        // for `<pluginPath>/plugins/<name>` as a fallback.
        let childDir: string | null = null
        const src = p.source
        if (typeof src === 'string') childDir = path.isAbsolute(src) ? src : path.join(pluginPath, src)
        else if (src && typeof src === 'object' && src.source === 'directory' && typeof src.path === 'string') {
          childDir = path.isAbsolute(src.path) ? src.path : path.join(pluginPath, src.path)
        }
        if (!childDir || !fs.existsSync(childDir)) {
          const guess = path.join(pluginPath, 'plugins', name)
          if (fs.existsSync(guess)) childDir = guess
        }
        const key = `${name}@${nestedName}`
        const counts = childDir ? countPluginContents(childDir, pluginPath) : { commands: 0, agents: 0, skills: 0, lspServers: 0, mcpServers: 0, hooks: 0 }
        counts.lspServers = childDir ? countPluginLspServers(name, nestedName, childDir) : 0
        let isCached = false
        if (installedKeys.has(key)) {
          try {
            const instData = JSON.parse(fs.readFileSync(installedFile, 'utf-8'))
            const ie = instData.plugins?.[key]?.[0]
            if (ie?.installPath && fs.existsSync(ie.installPath)) isCached = true
          } catch {}
        }
        results.push({
          name,
          version: p.version || '',
          description: p.description || '',
          author: p.author?.name || (typeof p.author === 'string' ? p.author : '') || mktData.owner?.name || '',
          marketplace: nestedName,
          keywords: Array.isArray(p.keywords) ? p.keywords : [],
          pluginPath: childDir || '',
          isInstalled: installedKeys.has(key),
          isCached,
          hasCommands: !!childDir && fs.existsSync(path.join(childDir, 'commands')),
          counts,
        })
      }
      return results
    } catch { return [] }
  })
}
