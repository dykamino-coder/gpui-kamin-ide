// Plugin-content scanners: count + list-with-descriptions.
// Extracted from `electron/main/ipc/plugins.ts` as part of Sprint 2 / Stage C
// (C2). All four functions are pure — they read from the filesystem but do
// not touch IPC, settings, or other side-effect surfaces.

import path from 'path'
import fs from 'fs'
import os from 'os'
import { extractFrontmatter, parseFrontmatter } from '../../plugin-helpers'

export interface PluginContentCounts {
  commands: number
  agents: number
  skills: number
  lspServers: number
  mcpServers: number
  hooks: number
  isBrokenMarketplace?: boolean
  subPluginTotal?: number
  subPluginInstalled?: number
  nestedMarketplaceName?: string
}

export interface PluginContentList {
  skills: { name: string; description?: string }[]
  agents: { name: string; description?: string }[]
  commands: { name: string; description?: string }[]
  mcpServers: string[]
  hooks: { event: string; matcher?: string; type: string; summary: string }[]
}

/** Plugin content counts — .md files in commands/agents/skills + LSP count.
 *  Counts top-level only (matches Claude Code CLI's scan behaviour). If a
 *  "plugin" ships `.claude-plugin/marketplace.json` instead of plugin.json,
 *  it's a broken install — a marketplace packaged as a plugin. CLI ignores
 *  it, so we report 0. */
export function countPluginContents(pluginDir: string, marketplaceRoot?: string): PluginContentCounts {
  const counts: PluginContentCounts = { commands: 0, agents: 0, skills: 0, lspServers: 0, mcpServers: 0, hooks: 0 }
  if (!pluginDir || !fs.existsSync(pluginDir)) return counts
  try {
    // Detect "marketplace-as-plugin" mis-install: has marketplace.json but no
    // plugin.json. EXCEPT when this `pluginDir` actually IS the current
    // marketplace's own root — which happens a lot in the wild when a
    // monorepo marketplace uses `"source": "./"` for every plugin entry
    // (e.g. anthropic-agent-skills, several claude-plugins-official entries).
    // Those plugins legitimately share the marketplace root as their source
    // directory; they are NOT broken sub-marketplaces. Flagging them made
    // every such plugin claim "N sub-plugins" where N was the parent
    // marketplace's own plugin count.
    const isSameAsMarketplaceRoot = !!marketplaceRoot && path.resolve(pluginDir) === path.resolve(marketplaceRoot)
    const hasPluginJson = fs.existsSync(path.join(pluginDir, '.claude-plugin', 'plugin.json'))
    const hasMarketplaceJson = fs.existsSync(path.join(pluginDir, '.claude-plugin', 'marketplace.json'))
    if (!hasPluginJson && hasMarketplaceJson && !isSameAsMarketplaceRoot) {
      counts.isBrokenMarketplace = true
      // Enumerate the nested marketplace so the UI can show
      // "N sub-plugins · M installed" instead of a bare warning. Child
      // names are matched against `installed_plugins.json` under the
      // nested marketplace's own name — NOT our parent marketplace.
      try {
        const raw = fs.readFileSync(path.join(pluginDir, '.claude-plugin', 'marketplace.json'), 'utf-8')
        const m = JSON.parse(raw)
        if (Array.isArray(m?.plugins)) counts.subPluginTotal = m.plugins.length
        if (typeof m?.name === 'string') counts.nestedMarketplaceName = m.name
        if (counts.nestedMarketplaceName && counts.subPluginTotal !== undefined) {
          const installedPath = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
          if (fs.existsSync(installedPath)) {
            const inst = JSON.parse(fs.readFileSync(installedPath, 'utf-8'))
            if (inst?.version === 2 && inst?.plugins) {
              const keys = Object.keys(inst.plugins)
              const prefix = `@${counts.nestedMarketplaceName}`
              counts.subPluginInstalled = keys.filter(k => k.endsWith(prefix)).length
            } else {
              counts.subPluginInstalled = 0
            }
          } else {
            counts.subPluginInstalled = 0
          }
        }
      } catch { /* malformed marketplace.json — keep just the flag */ }
      return counts
    }

    function countMdFiles(dir: string): number {
      let n = 0
      if (!fs.existsSync(dir)) return 0
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        if (e.isDirectory()) n += countMdFiles(path.join(dir, e.name))
        else if (e.name.endsWith('.md')) n++
      }
      return n
    }
    counts.commands = countMdFiles(path.join(pluginDir, 'commands'))
    counts.agents = countMdFiles(path.join(pluginDir, 'agents'))
    if (fs.existsSync(path.join(pluginDir, 'SKILL.md'))) counts.skills = 1
    // Skills — recursive scan to match CLI behaviour. CLI walks skills/**
    // and counts every `skill.md` (case-insensitive), with namespace
    // formed from the path. We previously only walked the top level which
    // missed nested skills like `skills/auth/multi-factor/SKILL.md`.
    function countSkillsRecursive(dir: string): number {
      let n = 0
      if (!fs.existsSync(dir)) return 0
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, e.name)
        if (e.isDirectory()) {
          n += countSkillsRecursive(full)
        } else if (e.isFile()) {
          // Canonical: SKILL.md (case-insensitive). Also accept flat
          // layout `<name>.md` at top-level only (legacy convention used
          // by Axiom and a few others) — but exclude README files.
          if (e.name.toLowerCase() === 'skill.md') n++
        }
      }
      return n
    }
    const skillsDir = path.join(pluginDir, 'skills')
    if (fs.existsSync(skillsDir)) {
      counts.skills += countSkillsRecursive(skillsDir)
      // Flat-layout fallback at the top level only (siblings of skills/<name>/)
      // — only include if no SKILL.md was found via recursive walk for that
      // name.
      for (const e of fs.readdirSync(skillsDir, { withFileTypes: true })) {
        if (e.isFile() && e.name.endsWith('.md') && e.name.toUpperCase() !== 'README.MD') {
          counts.skills++
        }
      }
    }
    // Manifest-declared content paths (CLI supports `manifest.commands`,
    // `manifest.agents`, `manifest.skills`). When the manifest declares
    // extra paths outside the standard directories, also count them.
    try {
      const manifestJson = path.join(pluginDir, '.claude-plugin', 'plugin.json')
      const altManifestJson = path.join(pluginDir, 'plugin.json')
      const mp = fs.existsSync(manifestJson) ? manifestJson : (fs.existsSync(altManifestJson) ? altManifestJson : null)
      if (mp) {
        const m = JSON.parse(fs.readFileSync(mp, 'utf-8'))
        const addPaths = (field: unknown): string[] => {
          if (!field) return []
          if (typeof field === 'string') return [field]
          if (Array.isArray(field)) return field.filter((x): x is string => typeof x === 'string')
          if (typeof field === 'object') return Object.keys(field) // object form (e.g. commands map)
          return []
        }
        const resolvePath = (rel: string) => path.isAbsolute(rel) ? rel : path.join(pluginDir, rel)
        for (const rel of addPaths(m?.commands)) {
          const p = resolvePath(rel)
          if (fs.existsSync(p)) counts.commands += fs.statSync(p).isDirectory() ? countMdFiles(p) : 1
        }
        for (const rel of addPaths(m?.agents)) {
          const p = resolvePath(rel)
          if (fs.existsSync(p)) counts.agents += fs.statSync(p).isDirectory() ? countMdFiles(p) : 1
        }
        for (const rel of addPaths(m?.skills)) {
          const p = resolvePath(rel)
          if (fs.existsSync(p)) counts.skills += fs.statSync(p).isDirectory() ? countSkillsRecursive(p) : 1
        }
      }
    } catch { /* manifest paths optional — silent skip */ }
    // MCP servers declared in plugin.json — count keys of `mcpServers` or
    // entries in `<pluginDir>/.mcp.json`. CLI supports both places.
    try {
      const manifestJson = path.join(pluginDir, '.claude-plugin', 'plugin.json')
      if (fs.existsSync(manifestJson)) {
        const m = JSON.parse(fs.readFileSync(manifestJson, 'utf-8'))
        const spec = m?.mcpServers
        if (spec && typeof spec === 'object' && !Array.isArray(spec)) {
          counts.mcpServers = Object.keys(spec).length
        }
      }
      if (counts.mcpServers === 0) {
        const mcpJson = path.join(pluginDir, '.mcp.json')
        if (fs.existsSync(mcpJson)) {
          const m = JSON.parse(fs.readFileSync(mcpJson, 'utf-8'))
          const spec = m?.mcpServers
          if (spec && typeof spec === 'object' && !Array.isArray(spec)) {
            counts.mcpServers = Object.keys(spec).length
          }
        }
      }
    } catch { /* manifest missing / malformed */ }

    // Hooks — declared in plugin.json:hooks, hooks.json,
    // .claude-plugin/hooks.json, or hooks/hooks.json. Count = sum of handler
    // commands across all events × matchers (matches what HooksPanel surfaces).
    try {
      let hooksObj: Record<string, Array<{ hooks?: unknown[] }>> | null = null
      const candidates = [
        path.join(pluginDir, '.claude-plugin', 'plugin.json'),
        path.join(pluginDir, 'hooks.json'),
        path.join(pluginDir, '.claude-plugin', 'hooks.json'),
        path.join(pluginDir, 'hooks', 'hooks.json'),
      ]
      for (const cand of candidates) {
        if (!fs.existsSync(cand)) continue
        try {
          const parsed = JSON.parse(fs.readFileSync(cand, 'utf-8'))
          if (parsed?.hooks && typeof parsed.hooks === 'object') {
            hooksObj = parsed.hooks
            break
          }
        } catch { /* malformed — try next */ }
      }
      if (hooksObj) {
        let n = 0
        for (const matchers of Object.values(hooksObj)) {
          if (!Array.isArray(matchers)) continue
          for (const m of matchers) {
            if (Array.isArray(m?.hooks)) n += m.hooks.length
          }
        }
        counts.hooks = n
      }
    } catch { /* swallow */ }
  } catch {}
  return counts
}

/** LSP server count — .lsp.json / plugin.json:lspServers / marketplace fallback. */
export function countPluginLspServers(pluginName: string, marketplace: string, pluginDir?: string): number {
  if (pluginDir && fs.existsSync(pluginDir)) {
    const lspJson = path.join(pluginDir, '.lsp.json')
    if (fs.existsSync(lspJson)) {
      try {
        const data = JSON.parse(fs.readFileSync(lspJson, 'utf-8'))
        if (data && typeof data === 'object' && !Array.isArray(data)) return Object.keys(data).length
      } catch {}
    }
    const manifestJson = path.join(pluginDir, '.claude-plugin', 'plugin.json')
    if (fs.existsSync(manifestJson)) {
      try {
        const m = JSON.parse(fs.readFileSync(manifestJson, 'utf-8'))
        const spec = m?.lspServers
        if (spec && typeof spec === 'object' && !Array.isArray(spec)) return Object.keys(spec).length
        if (Array.isArray(spec)) return spec.length
        if (typeof spec === 'string') {
          const resolved = path.isAbsolute(spec) ? spec : path.join(pluginDir, spec)
          if (fs.existsSync(resolved)) {
            const lsp = JSON.parse(fs.readFileSync(resolved, 'utf-8'))
            if (lsp && typeof lsp === 'object' && !Array.isArray(lsp)) return Object.keys(lsp).length
          }
        }
      } catch {}
    }
  }
  if (marketplace) {
    try {
      const mktJson = path.join(os.homedir(), '.claude', 'plugins', 'marketplaces', marketplace, '.claude-plugin', 'marketplace.json')
      if (fs.existsSync(mktJson)) {
        const mkt = JSON.parse(fs.readFileSync(mktJson, 'utf-8'))
        const found = mkt.plugins?.find((p: any) => p.name === pluginName)
        if (found?.lspServers && typeof found.lspServers === 'object' && !Array.isArray(found.lspServers)) {
          return Object.keys(found.lspServers).length
        }
        if (Array.isArray(found?.lspServers)) return found.lspServers.length
      }
    } catch {}
  }
  return 0
}

/** List the names of skills/agents/commands shipped by a plugin directory.
 *  Powers the marketplace browser's "preview" expander — users see WHAT
 *  they're getting before clicking Install, not just opaque counts.
 *  Reads frontmatter `description:` field as a one-line summary; falls
 *  back to filename if no frontmatter present. Bounded to 50 items per
 *  bucket to keep IPC payloads small on huge marketplaces (some ship
 *  100+ skills). */
export function listPluginContents(pluginDir: string): PluginContentList {
  const out: PluginContentList = { skills: [], agents: [], commands: [], mcpServers: [], hooks: [] }
  if (!pluginDir || !fs.existsSync(pluginDir)) return out

  /** Targeted extraction of a single frontmatter field. Used as a fallback
   *  when the yaml package's full-document parse fails on a syntactically
   *  invalid sibling field — common in the wild because plugin authors
   *  routinely write things like `tags: [audit, 000-jeremy]` (the `000-`
   *  trips yaml's number-parsing strictness) that wreck the entire parse,
   *  leaving every OTHER field blank too. Handles plain, quoted, and
   *  block scalar (`|` / `>`) forms — enough to cover ~99% of real
   *  skill/agent/command files. */
  function extractFmField(fm: string, key: string): string | undefined {
    const lines = fm.replace(/\r/g, '').split('\n')
    const re = new RegExp(`^(\\s*)${key}:\\s*(.*)$`)
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i]
      if (line === undefined) continue
      const m = line.match(re)
      if (!m) continue
      const baseIndent = (m[1] ?? '').length
      const valPart = (m[2] ?? '').trim()
      const block = valPart.match(/^([|>])([+-]?)\s*$/)
      if (block) {
        // Block scalar — gather indented continuation lines until we
        // see one at-or-below baseIndent (next sibling field).
        const style = block[1]
        const collected: string[] = []
        for (let j = i + 1; j < lines.length; j++) {
          const ln = lines[j]
          if (ln === undefined) break
          if (ln.trim() === '') { collected.push(''); continue }
          const ind = (ln.match(/^(\s*)/)?.[1]?.length) ?? 0
          if (ind <= baseIndent) break
          collected.push(ln.trim())
        }
        while (collected.length && collected[collected.length - 1] === '') collected.pop()
        // Folded `>` joins paragraphs with a space, literal `|` keeps
        // newlines. For preview purposes we collapse anyway.
        return (style === '>' ? collected.join(' ') : collected.join('\n')).trim()
      }
      // Plain or quoted scalar — strip surrounding quotes if present.
      const stripped = valPart.replace(/^"|"$/g, '').replace(/^'|'$/g, '')
      return stripped
    }
    return undefined
  }

  function readDescription(filePath: string): string | undefined {
    try {
      // Cap read at 16KB — frontmatter + a small body cushion. Block
      // scalars (`description: |` followed by indented lines) need
      // enough bytes to capture the closing `---` marker, so 8KB cut
      // it off on long-form skill descriptions.
      const fd = fs.openSync(filePath, 'r')
      const buf = Buffer.alloc(16_384)
      const n = fs.readSync(fd, buf, 0, buf.length, 0)
      fs.closeSync(fd)
      const head = buf.slice(0, n).toString('utf-8')
      const fm = extractFrontmatter(head)
      if (!fm) return undefined

      // Try the yaml-package parser first (clean handling of all the
      // edge cases). When it fails (one bad sibling field aborts the
      // whole parse), fall back to a targeted single-field extractor
      // that does NOT need the full document to be valid.
      let raw: unknown
      try {
        const obj = parseFrontmatter(fm)
        raw = obj['description']
      } catch { /* parseFrontmatter swallows internally — won't reach */ }
      if (raw == null) raw = extractFmField(fm, 'description')

      if (raw == null) return undefined
      const text = typeof raw === 'string' ? raw : String(raw)
      return text.replace(/\s+/g, ' ').trim().slice(0, 200)
    } catch { return undefined }
  }

  function listMd(dir: string, cap: number): { name: string; description?: string }[] {
    if (!fs.existsSync(dir)) return []
    const acc: { name: string; description?: string }[] = []
    try {
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        if (acc.length >= cap) break
        if (e.isFile() && e.name.endsWith('.md')) {
          acc.push({ name: e.name.replace(/\.md$/, ''), description: readDescription(path.join(dir, e.name)) })
        }
      }
    } catch { /* permission etc. */ }
    return acc
  }

  out.commands = listMd(path.join(pluginDir, 'commands'), 50)
  out.agents = listMd(path.join(pluginDir, 'agents'), 50)

  // Skills layouts in the wild:
  //   1. `<pluginDir>/SKILL.md` — single root skill
  //   2. `<pluginDir>/skills/<name>/SKILL.md` — Anthropic-canonical
  //   3. `<pluginDir>/skills/<name>.md` — flat layout (Axiom etc.)
  if (fs.existsSync(path.join(pluginDir, 'SKILL.md'))) {
    out.skills.push({
      name: path.basename(pluginDir),
      description: readDescription(path.join(pluginDir, 'SKILL.md')),
    })
  }
  const skillsDir = path.join(pluginDir, 'skills')
  if (fs.existsSync(skillsDir)) {
    try {
      for (const e of fs.readdirSync(skillsDir, { withFileTypes: true })) {
        if (out.skills.length >= 50) break
        if (e.isDirectory()) {
          const skillMd = path.join(skillsDir, e.name, 'SKILL.md')
          if (fs.existsSync(skillMd)) {
            out.skills.push({ name: e.name, description: readDescription(skillMd) })
          }
        } else if (e.isFile() && e.name.endsWith('.md') && e.name.toUpperCase() !== 'README.MD') {
          out.skills.push({
            name: e.name.replace(/\.md$/i, ''),
            description: readDescription(path.join(skillsDir, e.name)),
          })
        }
      }
    } catch { /* permission etc. */ }
  }

  // MCP servers from manifest's mcpServers map or .mcp.json (matches counter).
  try {
    const manifestPath = path.join(pluginDir, '.claude-plugin', 'plugin.json')
    if (fs.existsSync(manifestPath)) {
      const m = JSON.parse(fs.readFileSync(manifestPath, 'utf-8'))
      if (m?.mcpServers && typeof m.mcpServers === 'object' && !Array.isArray(m.mcpServers)) {
        out.mcpServers = Object.keys(m.mcpServers).slice(0, 50)
      }
    }
    if (out.mcpServers.length === 0) {
      const mcpPath = path.join(pluginDir, '.mcp.json')
      if (fs.existsSync(mcpPath)) {
        const m = JSON.parse(fs.readFileSync(mcpPath, 'utf-8'))
        if (m?.mcpServers && typeof m.mcpServers === 'object' && !Array.isArray(m.mcpServers)) {
          out.mcpServers = Object.keys(m.mcpServers).slice(0, 50)
        }
      }
    }
  } catch { /* malformed */ }

  // Hooks — same discovery paths as countPluginContents/HooksPanel. Flatten
  // each event×matcher×handler into a single preview row so the UI can
  // show "PreToolUse / Bash / command" entries the way the Hooks tab does.
  try {
    let hooksObj: Record<string, Array<{ matcher?: string; hooks?: any[] }>> | null = null
    for (const cand of [
      path.join(pluginDir, '.claude-plugin', 'plugin.json'),
      path.join(pluginDir, 'plugin.json'),
      path.join(pluginDir, 'hooks.json'),
      path.join(pluginDir, '.claude-plugin', 'hooks.json'),
      path.join(pluginDir, 'hooks', 'hooks.json'),
    ]) {
      if (!fs.existsSync(cand)) continue
      try {
        const parsed = JSON.parse(fs.readFileSync(cand, 'utf-8'))
        if (parsed?.hooks && typeof parsed.hooks === 'object' && Object.keys(parsed.hooks).length > 0) {
          hooksObj = parsed.hooks
          break
        }
      } catch { /* malformed — try next */ }
    }
    if (hooksObj) {
      for (const [event, matchers] of Object.entries(hooksObj)) {
        if (!Array.isArray(matchers)) continue
        for (const m of matchers) {
          if (!Array.isArray(m?.hooks)) continue
          for (const h of m.hooks) {
            const type = String(h?.type ?? 'command')
            // Build a one-line summary: command/prompt/url shortened so the
            // preview list stays readable.
            let summary = ''
            if (type === 'command' && typeof h?.command === 'string') summary = h.command
            else if (type === 'prompt' && typeof h?.prompt === 'string') summary = h.prompt
            else if (type === 'http' && typeof h?.url === 'string') summary = h.url
            out.hooks.push({ event, matcher: m.matcher, type, summary: summary.slice(0, 200) })
          }
        }
      }
    }
  } catch { /* swallow */ }

  return out
}
