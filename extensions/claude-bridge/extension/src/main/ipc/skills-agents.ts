import { ipcMain, shell, type IpcMainInvokeEvent } from 'electron'
import path from 'path'
import fs from 'fs'
import os from 'os'
import {
  loadEnabledPluginsMap,
  decodeYamlScalar,
  parseYamlList,
  parseYamlBool,
  parseYamlInt,
  extractFrontmatter,
  firstBodyLine,
  matchYamlField,
  readPluginManifest,
} from '../plugin-helpers'
import type { TabManager } from '../tab-manager'

export interface SkillsAgentsContext {
  getTabManager: () => TabManager | null
  getUserCwd: () => string | null
}

export function registerSkillsAgentsIPC(ctx: SkillsAgentsContext): void {
  ipcMain.handle('skills:list', async () => {
    type SkillRow = {
      name: string
      fileName: string
      description: string
      path: string
      source: string
      /** Raw `name:` field from frontmatter (if author supplied one). CLI's
       *  `userFacingName()` falls back to this and `findCommand` matches it,
       *  so when present we prefer it as the user-visible slash form
       *  (mirrors what the CLI's own typeahead inserts). */
      displayName?: string
      argumentHint?: string
      shell?: string
      version?: string
      whenToUse?: string
      disableModelInvocation?: boolean
    }

    function parseSkillContent(filePath: string, fileName: string, source: string, content: string, overrideName?: string): SkillRow | null {
      const fm = extractFrontmatter(content) ?? ''
      const match = (key: string): string | undefined => matchYamlField(fm, key)
      const userInvocable = match('user-invocable')
      if (userInvocable && parseYamlBool(userInvocable) === false) return null

      const rawName = match('name')?.trim().replace(/^["']|["']$/g, '')
      const name = overrideName ?? rawName ?? fileName.replace(/\.md$/, '')
      let description = match('description')
      description = description ? decodeYamlScalar(description) : firstBodyLine(content, fileName)

      const row: SkillRow = {
        name, fileName, description, path: filePath, source,
      }
      if (rawName) row.displayName = rawName
      const argumentHint = match('argument-hint') ?? match('argumentHint')
      if (argumentHint) row.argumentHint = decodeYamlScalar(argumentHint)
      const shellMeta = match('shell')
      if (shellMeta) row.shell = decodeYamlScalar(shellMeta)
      const version = match('version')
      if (version) row.version = decodeYamlScalar(version)
      const whenToUse = match('when_to_use') ?? match('whenToUse')
      if (whenToUse) row.whenToUse = decodeYamlScalar(whenToUse)
      const disableModelInvocation = match('disable-model-invocation') ?? match('disableModelInvocation')
      if (disableModelInvocation) {
        const b = parseYamlBool(disableModelInvocation)
        if (b !== null) row.disableModelInvocation = b
      }
      return row
    }

    async function safeReadFile(p: string): Promise<string | null> {
      try { return await fs.promises.readFile(p, 'utf-8') } catch { return null }
    }

    const enabledPlugins = await loadEnabledPluginsMap()

    const cwd = ctx.getUserCwd() || process.cwd()
    const commandsDir = path.join(cwd, '.claude', 'commands')
    const projectPromise = (async (): Promise<SkillRow[]> => {
      let files: string[]
      try { files = (await fs.promises.readdir(commandsDir)).filter(f => f.endsWith('.md')) }
      catch { return [] }
      const rows = await Promise.all(files.map(async (f): Promise<SkillRow | null> => {
        const filePath = path.join(commandsDir, f)
        const content = await safeReadFile(filePath)
        if (content === null) return null
        return parseSkillContent(filePath, f, 'project', content)
      }))
      return rows.filter((r): r is SkillRow => r !== null)
    })()

    const userSkillsDir = path.join(os.homedir(), '.claude', 'skills')
    const userPromise = (async (): Promise<SkillRow[]> => {
      let dirs: import('fs').Dirent[]
      try { dirs = (await fs.promises.readdir(userSkillsDir, { withFileTypes: true })).filter(d => d.isDirectory()) }
      catch { return [] }
      const rows = await Promise.all(dirs.map(async (d): Promise<SkillRow | null> => {
        const skillMd = path.join(userSkillsDir, d.name, 'SKILL.md')
        const content = await safeReadFile(skillMd)
        if (content === null) return null
        return parseSkillContent(skillMd, d.name, 'user', content, d.name)
      }))
      return rows.filter((r): r is SkillRow => r !== null)
    })()

    const pluginsFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    const pluginsPromise = (async (): Promise<SkillRow[]> => {
      const raw = await safeReadFile(pluginsFile)
      if (!raw) return []
      let data: any
      try { data = JSON.parse(raw) } catch { return [] }
      if (data.version !== 2 || !data.plugins) return []

      async function scanCommandsDir(dir: string, prefix = ''): Promise<{ relName: string; filePath: string }[]> {
        let entries: import('fs').Dirent[]
        try { entries = await fs.promises.readdir(dir, { withFileTypes: true }) }
        catch { return [] }
        const nested = await Promise.all(entries.map(async (e) => {
          if (e.isDirectory()) {
            return scanCommandsDir(path.join(dir, e.name), prefix ? `${prefix}/${e.name}` : e.name)
          }
          if (e.name.endsWith('.md')) {
            const name = e.name.replace(/\.md$/, '')
            return [{ relName: prefix ? `${prefix}/${name}` : name, filePath: path.join(dir, e.name) }]
          }
          return [] as { relName: string; filePath: string }[]
        }))
        return nested.flat()
      }

      const perPlugin = await Promise.all(
        Object.entries(data.plugins as Record<string, any[]>).map(async ([key, entries]) => {
          if (enabledPlugins.get(key) === false) return []
          const [pluginName] = key.split('@')
          const entry = entries[0]
          if (!entry?.installPath) return []
          const pluginRoot = entry.installPath
          const rows: SkillRow[] = []

          const commandsDirLocal = path.join(pluginRoot, 'commands')
          const cmds = await scanCommandsDir(commandsDirLocal)
          const scanRows = await Promise.all(cmds.map(async (cmd): Promise<SkillRow | null> => {
            const content = await safeReadFile(cmd.filePath)
            if (content === null) return null
            return parseSkillContent(
              cmd.filePath,
              path.basename(cmd.filePath),
              `plugin:${pluginName}`,
              content,
              cmd.relName,
            )
          }))
          for (const r of scanRows) if (r) rows.push(r)

          const pluginSkillsDir = path.join(pluginRoot, 'skills')
          let skillDirs: import('fs').Dirent[] = []
          try {
            skillDirs = (await fs.promises.readdir(pluginSkillsDir, { withFileTypes: true })).filter(d => d.isDirectory())
          } catch { /* dir absent */ }
          const skillRows = await Promise.all(skillDirs.map(async (d): Promise<SkillRow | null> => {
            const skillMd = path.join(pluginSkillsDir, d.name, 'SKILL.md')
            const content = await safeReadFile(skillMd)
            if (content === null) return null
            return parseSkillContent(skillMd, d.name, `plugin:${pluginName}`, content, d.name)
          }))
          for (const r of skillRows) if (r) rows.push(r)

          const manifest = await readPluginManifest(pluginRoot)
          const spec = manifest?.commands
          const items: unknown[] = Array.isArray(spec) ? spec
            : (typeof spec === 'string' || (spec && typeof spec === 'object' && !Array.isArray(spec))) ? [spec]
            : []
          const seenPaths = new Set(scanRows.filter(Boolean).map(r => (r as SkillRow).path))
          for (const item of items) {
            if (typeof item === 'string') {
              const resolved = path.isAbsolute(item) ? item : path.join(pluginRoot, item)
              const content = await safeReadFile(resolved)
              if (content === null) continue
              if (seenPaths.has(resolved)) continue
              const parsed = parseSkillContent(resolved, path.basename(resolved), `plugin:${pluginName}`, content)
              if (parsed) { rows.push(parsed); seenPaths.add(resolved) }
            } else if (item && typeof item === 'object' && !Array.isArray(item)) {
              for (const [cmdName, meta] of Object.entries(item as Record<string, any>)) {
                if (!meta || typeof meta !== 'object') continue
                const srcPath = typeof meta.source === 'string'
                  ? (path.isAbsolute(meta.source) ? meta.source : path.join(pluginRoot, meta.source))
                  : null
                let content: string | null = null
                if (srcPath) content = await safeReadFile(srcPath)
                else if (typeof meta.content === 'string') content = meta.content
                if (content === null) continue
                const row = parseSkillContent(
                  srcPath ?? path.join(pluginRoot, `${cmdName}.md`),
                  srcPath ? path.basename(srcPath) : `${cmdName}.md`,
                  `plugin:${pluginName}`,
                  content,
                  cmdName,
                )
                if (!row) continue
                if (typeof meta.description === 'string') row.description = meta.description
                if (typeof meta['argument-hint'] === 'string') row.argumentHint = meta['argument-hint']
                if (typeof meta.shell === 'string') row.shell = meta.shell
                if (typeof meta.version === 'string') row.version = meta.version
                rows.push(row)
              }
            }
          }
          return rows
        }),
      )
      return perPlugin.flat()
    })()

    const [project, user, plugins] = await Promise.all([projectPromise, userPromise, pluginsPromise])
    return [...project, ...user, ...plugins]
  })

  // List subagents available to the CLI.
  ipcMain.handle('agents:list', async () => {
    interface AgentInfo {
      name: string
      fileName: string
      description: string
      model: string | null
      tools: string[]
      path: string
      source: string
      color?: string
      effort?: string | number
      maxTurns?: number
      disallowedTools?: string[]
      memory?: string
      isolation?: string
      background?: boolean
      skills?: string[]
    }

    async function parseAgent(filePath: string, source: string, overrideName?: string, contentOverride?: string): Promise<AgentInfo | null> {
      let content: string
      if (typeof contentOverride === 'string') content = contentOverride
      else {
        try { content = await fs.promises.readFile(filePath, 'utf-8') } catch { return null }
      }
      const fm = extractFrontmatter(content) ?? ''
      const match = (key: string): string | undefined => matchYamlField(fm, key)
      const fileName = path.basename(filePath)
      let name = overrideName ?? fileName.replace(/\.md$/, '')
      let description = ''
      let model: string | null = null
      let tools: string[] = []

      const nameMatch = match('name')
      const descMatch = match('description')
      const whenMatch = match('when-to-use') ?? match('when_to_use')
      const modelMatch = match('model')
      const toolsMatch = match('tools')
      if (nameMatch && !overrideName) name = decodeYamlScalar(nameMatch)
      if (descMatch) description = decodeYamlScalar(descMatch)
      if (!description && whenMatch) description = decodeYamlScalar(whenMatch)
      if (modelMatch) model = decodeYamlScalar(modelMatch)
      if (toolsMatch) tools = parseYamlList(toolsMatch)

      if (!description) description = firstBodyLine(content, '')

      const info: AgentInfo = { name, fileName, description, model, tools, path: filePath, source }

      const colorMatch = match('color')
      if (colorMatch) info.color = decodeYamlScalar(colorMatch)
      const effortMatch = match('effort')
      if (effortMatch) {
        const n = parseYamlInt(effortMatch)
        info.effort = n !== null ? n : decodeYamlScalar(effortMatch)
      }
      const maxTurnsMatch = match('maxTurns') ?? match('max-turns')
      if (maxTurnsMatch) {
        const n = parseYamlInt(maxTurnsMatch)
        if (n !== null) info.maxTurns = n
      }
      const disallowedMatch = match('disallowedTools') ?? match('disallowed-tools')
      if (disallowedMatch) info.disallowedTools = parseYamlList(disallowedMatch)
      const memoryMatch = match('memory')
      if (memoryMatch) info.memory = decodeYamlScalar(memoryMatch)
      const isolationMatch = match('isolation')
      if (isolationMatch) info.isolation = decodeYamlScalar(isolationMatch)
      const backgroundMatch = match('background')
      if (backgroundMatch) {
        const b = parseYamlBool(backgroundMatch)
        if (b !== null) info.background = b
      }
      const skillsMatch = match('skills')
      if (skillsMatch) info.skills = parseYamlList(skillsMatch)
      return info
    }

    async function scanAgentsDir(dir: string, source: string): Promise<AgentInfo[]> {
      let files: import('fs').Dirent[]
      try { files = await fs.promises.readdir(dir, { withFileTypes: true }) }
      catch { return [] }
      const mdFiles = files.filter(f => f.isFile() && f.name.endsWith('.md'))
      const parsed = await Promise.all(mdFiles.map(f => parseAgent(path.join(dir, f.name), source)))
      return parsed.filter((p): p is AgentInfo => p !== null)
    }

    const enabledPluginsForAgents = await loadEnabledPluginsMap()
    const tm = ctx.getTabManager()

    const projectCwds = new Set<string>()
    const userCwd = ctx.getUserCwd()
    if (userCwd) projectCwds.add(userCwd)
    try {
      for (const cwd of tm?.getDistinctCwds() ?? []) projectCwds.add(cwd)
    } catch { /* tabManager may not be ready */ }
    const projectPromise = Promise.all(
      [...projectCwds].map(cwd => scanAgentsDir(path.join(cwd, '.claude', 'agents'), 'project')),
    ).then(arrs => arrs.flat())

    const userPromise = scanAgentsDir(path.join(os.homedir(), '.claude', 'agents'), 'user')

    const pluginsPromise = (async (): Promise<AgentInfo[]> => {
      const pluginsFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
      let raw: string
      try { raw = await fs.promises.readFile(pluginsFile, 'utf-8') } catch { return [] }
      let data: any
      try { data = JSON.parse(raw) } catch { return [] }
      if (data.version !== 2 || !data.plugins) return []

      const perPlugin = await Promise.all(
        Object.entries(data.plugins as Record<string, any[]>).map(async ([key, entries]) => {
          if (enabledPluginsForAgents.get(key) === false) return []
          const [pluginName] = key.split('@')
          const entry = entries[0]
          if (!entry?.installPath) return []
          const pluginRoot = entry.installPath
          const scanned = await scanAgentsDir(path.join(pluginRoot, 'agents'), `plugin:${pluginName}`)
          const seenPaths = new Set(scanned.map(a => a.path))

          const manifest = await readPluginManifest(pluginRoot)
          const spec = manifest?.agents
          const items: unknown[] = Array.isArray(spec) ? spec
            : (typeof spec === 'string' || (spec && typeof spec === 'object' && !Array.isArray(spec))) ? [spec]
            : []
          for (const item of items) {
            if (typeof item === 'string') {
              const resolved = path.isAbsolute(item) ? item : path.join(pluginRoot, item)
              if (seenPaths.has(resolved)) continue
              const parsed = await parseAgent(resolved, `plugin:${pluginName}`)
              if (parsed) { scanned.push(parsed); seenPaths.add(resolved) }
            } else if (item && typeof item === 'object' && !Array.isArray(item)) {
              for (const [agentName, meta] of Object.entries(item as Record<string, any>)) {
                if (!meta || typeof meta !== 'object') continue
                const srcPath = typeof meta.source === 'string'
                  ? (path.isAbsolute(meta.source) ? meta.source : path.join(pluginRoot, meta.source))
                  : null
                const contentOverride = typeof meta.content === 'string' ? meta.content : undefined
                const parsed = await parseAgent(
                  srcPath ?? path.join(pluginRoot, `${agentName}.md`),
                  `plugin:${pluginName}`,
                  agentName,
                  contentOverride,
                )
                if (!parsed) continue
                if (typeof meta.description === 'string') parsed.description = meta.description
                if (typeof meta.model === 'string') parsed.model = meta.model
                if (typeof meta.color === 'string') parsed.color = meta.color
                scanned.push(parsed)
              }
            }
          }
          return scanned
        }),
      )
      return perPlugin.flat()
    })()

    const [project, user, plugins] = await Promise.all([projectPromise, userPromise, pluginsPromise])
    return [...project, ...user, ...plugins]
  })

  // Plugin-shipped output styles.
  ipcMain.handle('output-styles:list', async () => {
    type StyleRow = { name: string; description: string; path: string; source: string }
    const rows: StyleRow[] = []
    const enabled = await loadEnabledPluginsMap()
    const pluginsFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    let raw: string
    try { raw = await fs.promises.readFile(pluginsFile, 'utf-8') } catch { return [] }
    let data: any
    try { data = JSON.parse(raw) } catch { return [] }
    if (data.version !== 2 || !data.plugins) return []

    async function readMdMeta(p: string): Promise<{ description: string } | null> {
      let content: string
      try { content = await fs.promises.readFile(p, 'utf-8') } catch { return null }
      const fm = extractFrontmatter(content) ?? ''
      const desc = matchYamlField(fm, 'description')
      return {
        description: desc || firstBodyLine(content, ''),
      }
    }

    for (const [key, entries] of Object.entries(data.plugins as Record<string, any[]>)) {
      if (enabled.get(key) === false) continue
      const [pluginName] = key.split('@')
      const entry = entries[0]
      if (!entry?.installPath) continue
      const pluginRoot = entry.installPath
      const source = `plugin:${pluginName}`

      const stylesDir = path.join(pluginRoot, 'output-styles')
      try {
        const files = await fs.promises.readdir(stylesDir, { withFileTypes: true })
        for (const f of files) {
          if (!f.isFile() || !f.name.endsWith('.md')) continue
          const fp = path.join(stylesDir, f.name)
          const meta = await readMdMeta(fp)
          if (!meta) continue
          rows.push({ name: f.name.replace(/\.md$/, ''), description: meta.description, path: fp, source })
        }
      } catch { /* no dir */ }

      const manifest = await readPluginManifest(pluginRoot)
      const spec = manifest?.outputStyles
      const items: unknown[] = Array.isArray(spec) ? spec
        : (typeof spec === 'string' || (spec && typeof spec === 'object' && !Array.isArray(spec))) ? [spec]
        : []
      for (const item of items) {
        if (typeof item === 'string') {
          const resolved = path.isAbsolute(item) ? item : path.join(pluginRoot, item)
          const meta = await readMdMeta(resolved)
          if (!meta) continue
          rows.push({ name: path.basename(resolved, '.md'), description: meta.description, path: resolved, source })
        } else if (item && typeof item === 'object' && !Array.isArray(item)) {
          for (const [styleName, m] of Object.entries(item as Record<string, any>)) {
            if (!m || typeof m !== 'object') continue
            const srcPath = typeof m.source === 'string'
              ? (path.isAbsolute(m.source) ? m.source : path.join(pluginRoot, m.source))
              : null
            const description = typeof m.description === 'string' ? m.description
              : (srcPath ? (await readMdMeta(srcPath))?.description ?? '' : '')
            rows.push({
              name: styleName,
              description,
              path: srcPath ?? path.join(pluginRoot, `${styleName}.md`),
              source,
            })
          }
        }
      }
    }
    return rows
  })

  ipcMain.handle('skills:create', (_event: IpcMainInvokeEvent, name: string, content: string) => {
    const cwd = ctx.getUserCwd() || process.cwd()
    const skillsDir = path.join(cwd, '.claude', 'commands')
    fs.mkdirSync(skillsDir, { recursive: true })
    const fileName = name.replace(/[^a-zA-Z0-9_-]/g, '-') + '.md'
    const filePath = path.join(skillsDir, fileName)
    fs.writeFileSync(filePath, content, 'utf-8')
    return { name, fileName, path: filePath }
  })

  ipcMain.handle('skills:delete', (_event: IpcMainInvokeEvent, fileName: string) => {
    const cwd = ctx.getUserCwd() || process.cwd()
    const filePath = path.join(cwd, '.claude', 'commands', fileName)
    if (fs.existsSync(filePath)) fs.unlinkSync(filePath)
  })

  ipcMain.handle('skills:open', (_event: IpcMainInvokeEvent, filePath: string) => {
    shell.openPath(filePath)
  })

  ipcMain.handle('skills:show-in-folder', (_event: IpcMainInvokeEvent, filePath: string) => {
    shell.showItemInFolder(filePath)
  })

  ipcMain.handle('skills:read', (_event: IpcMainInvokeEvent, filePath: string) => {
    try {
      return fs.readFileSync(filePath, 'utf-8')
    } catch {
      return null
    }
  })
}
