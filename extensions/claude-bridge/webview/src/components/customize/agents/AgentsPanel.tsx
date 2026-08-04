import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { readPanelCache, writePanelCache } from '../../../lib/panel-cache'
import { ListSkeleton } from '../../ui/ListSkeleton'
import { PanelError } from '../../ui/PanelError'
import { ThreeColumnPanel } from '../shared/ThreeColumnPanel'
import { ThreeColumnList } from '../shared/ThreeColumnList'
import { ThreeColumnDetail } from '../shared/ThreeColumnDetail'
import { AgentDescription } from './AgentDescription'
import { SkillContent } from '../skills/SkillContent'
import skillItemStyles from '../skills/SkillItem.module.css'
import { prettifyPluginName } from '../../../utils/prettify-plugin-name'

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
}

// Map Claude's agent-color tokens to visible hex accents. Fallback to the
// catppuccin green if the plugin author used a non-standard value.
const AGENT_COLORS: Record<string, string> = {
  red: 'var(--accent-red)', orange: 'var(--accent-orange)', yellow: 'var(--accent-yellow)', green: 'var(--accent-green)',
  teal: 'var(--accent-teal)', blue: 'var(--accent-primary)', purple: 'var(--accent-purple)', pink: 'var(--accent-pink)',
  gray: 'var(--text-secondary)', grey: 'var(--text-secondary)',
}
function agentAccent(color: string | undefined): string | null {
  if (!color) return null
  const lower = color.toLowerCase()
  return AGENT_COLORS[lower] ?? (lower.startsWith('#') ? lower : null)
}

function sourceBadgeLabel(source: string): string {
  if (source === 'user') return 'User'
  if (source === 'project') return 'Project'
  if (source.startsWith('plugin:')) return prettifyPluginName(source.slice('plugin:'.length))
  return source
}

function groupLabel(source: string): string {
  if (source === 'project') return 'Project agents'
  if (source === 'user') return 'User agents'
  if (source.startsWith('plugin:')) return prettifyPluginName(source.slice('plugin:'.length))
  return 'Other'
}

function groupOrder(source: string): number {
  if (source === 'project') return 0
  if (source === 'user') return 1
  if (source.startsWith('plugin:')) return 2
  return 3
}

function AgentListItem({
  agent,
  isActive,
  onClick,
}: {
  agent: AgentInfo
  isActive: boolean
  onClick: () => void
}): JSX.Element {
  const accent = agentAccent(agent.color)
  const iconColor = accent ?? 'var(--text-disabled)'
  return (
    <div
      class={`${skillItemStyles.item} ${isActive ? skillItemStyles.active : ''}`}
      onClick={onClick}
    >
      <div class={skillItemStyles.icon} style={accent ? `box-shadow:inset 0 0 0 1px color-mix(in srgb, ${accent} 20%, transparent)` : undefined}>
        <i class="fas fa-user-astronaut" style={`color:${iconColor}`} />
      </div>
      <div class={skillItemStyles.info}>
        <div class={skillItemStyles.name}>{agent.name}</div>
        <div class={skillItemStyles.desc}>{agent.description || agent.fileName}</div>
      </div>
      {agent.background && (
        <i class="fas fa-moon" data-tooltip="Background agent" style="font-size:10px;color:var(--text-muted);flex-shrink:0" />
      )}
    </div>
  )
}

function AgentBody({ path, body }: { path: string; body: string | null | undefined }): JSX.Element {
  if (body === undefined) {
    return <div style="padding:12px 0;color:var(--text-muted);font-size:12px;font-style:italic">Loading…</div>
  }
  if (!body) {
    return <div style="padding:12px 0;color:var(--text-muted);font-size:12px;font-style:italic">No body content.</div>
  }
  return <SkillContent key={path} content={body} />
}

export function AgentsPanel(): JSX.Element {
  const bridge = useBridge()
  const [agents, setAgents] = useState<AgentInfo[]>([])
  const [filter, setFilter] = useState('')
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [bodyCache, setBodyCache] = useState<Map<string, string | null>>(new Map())
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)

  async function refresh(): Promise<void> {
    setLoadError(false)
    try {
      const result = await bridge.listAgents()
      const list = result ?? []
      setAgents(list)
      writePanelCache('agents.list.v1', list)
    } catch {
      setLoadError(true)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    // Cache-first: paint the last-known list instantly (clears loading), then
    // revalidate. Only cold opens (no cache) show the skeleton.
    const cached = readPanelCache<AgentInfo[]>('agents.list.v1')
    if (cached) { setAgents(cached); setLoading(false) }
    void refresh()
  }, [bridge])

  // Lazy-load the markdown body (everything after frontmatter) when the user
  // selects an agent. Cached per path so switching back is instant.
  useEffect(() => {
    if (!selectedPath) return
    if (bodyCache.has(selectedPath)) return
    let cancelled = false
    bridge.readSkill(selectedPath).then(raw => {
      if (cancelled) return
      const body = (raw ?? '').replace(/^---\s*\n[\s\S]*?\n---\s*\n?/, '').trim()
      setBodyCache(prev => {
        const next = new Map(prev)
        next.set(selectedPath, body || null)
        return next
      })
    }).catch(() => {
      setBodyCache(prev => {
        const next = new Map(prev)
        next.set(selectedPath, null)
        return next
      })
    })
    return () => { cancelled = true }
  }, [selectedPath, bridge])

  const q = filter.toLowerCase()
  const filtered = agents.filter(a =>
    !q || a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q),
  )

  // Group by source and sort groups so Project → User → plugins → other.
  const groupsMap = new Map<string, AgentInfo[]>()
  for (const a of filtered) {
    const key = groupLabel(a.source)
    const list = groupsMap.get(key) ?? []
    list.push(a)
    groupsMap.set(key, list)
  }
  const groups = [...groupsMap.entries()].sort((a, b) => {
    const sa = filtered.find(x => groupLabel(x.source) === a[0])?.source ?? ''
    const sb = filtered.find(x => groupLabel(x.source) === b[0])?.source ?? ''
    const orderDiff = groupOrder(sa) - groupOrder(sb)
    if (orderDiff !== 0) return orderDiff
    return a[0].localeCompare(b[0])
  })

  const selected = agents.find(a => a.path === selectedPath) ?? null

  return (
    <ThreeColumnPanel>
      <ThreeColumnList
        title="Agents"
        searchValue={filter}
        onSearchChange={setFilter}
        searchPlaceholder="Search agents..."
      >
        {loading && agents.length === 0 ? (
          <ListSkeleton rows={7} />
        ) : loadError && agents.length === 0 ? (
          <PanelError message="Could not load agents." onRetry={() => { setLoading(true); void refresh() }} />
        ) : filtered.length === 0 ? (
          <div style="padding:16px;color:var(--text-disabled);font-size:12px">No agents found.</div>
        ) : (
          <>
            {groups.map(([groupName, items]) => (
              <div key={groupName}>
                <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.5px;color:var(--text-disabled);padding:12px 16px 4px">{groupName}</div>
                {items.map(a => (
                  <AgentListItem
                    key={a.path}
                    agent={a}
                    isActive={a.path === selectedPath}
                    onClick={() => setSelectedPath(a.path)}
                  />
                ))}
              </div>
            ))}
          </>
        )}
      </ThreeColumnList>

      <ThreeColumnDetail>
        {!selected ? (
          <div style="padding:24px;color:var(--text-muted)">Select an agent to view details.</div>
        ) : (
          <div style="padding:20px;overflow-y:auto;height:100%">
            <div style="margin-bottom:12px">
              <h2 style="font-size: var(--fs-lg);font-weight:600;color:var(--text-primary);margin:0 0 4px">{selected.name}</h2>
              <div style="display:flex;align-items:center;gap:8px;font-size:12px;color:var(--text-disabled);margin-top:4px;flex-wrap:wrap">
                <span style="padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;background:var(--bg-surface);color:var(--accent-primary)">{sourceBadgeLabel(selected.source)}</span>
                {selected.model && (
                  <span style="padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;background:var(--bg-surface);color:var(--accent-purple)">model: {selected.model}</span>
                )}
                {selected.effort !== undefined && (
                  <span style="padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;background:var(--bg-surface);color:var(--accent-yellow)">effort: {String(selected.effort)}</span>
                )}
                {selected.maxTurns !== undefined && (
                  <span style="padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;background:var(--bg-surface);color:var(--accent-orange)">maxTurns: {selected.maxTurns}</span>
                )}
                {selected.color && agentAccent(selected.color) && (
                  <span style={`padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;background:var(--bg-surface);color:${agentAccent(selected.color)}`}>color: {selected.color}</span>
                )}
                {selected.memory && (
                  <span style="padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;background:var(--bg-surface);color:var(--accent-teal)">memory: {selected.memory}</span>
                )}
                {selected.isolation && (
                  <span style="padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;background:var(--bg-surface);color:var(--accent-pink)">isolation: {selected.isolation}</span>
                )}
                {selected.background && (
                  <span style="padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;background:var(--bg-surface);color:var(--text-secondary)">background</span>
                )}
                <span>{selected.fileName}</span>
              </div>
            </div>

            {selected.description && (
              <div style="margin:0 0 12px">
                <AgentDescription text={selected.description} />
              </div>
            )}

            <div style="display:flex;gap:8px;margin-bottom:16px;flex-wrap:wrap">
              <button
                style="padding:6px 14px;border-radius:var(--radius-sm);font-size:12px;cursor:pointer;border:1px solid var(--accent-action);background:var(--accent-action);color:var(--accent-action-fg);font-weight:500;transition:opacity .15s"
                onClick={() => bridge.openSkill(selected.path)}
              >
                Open in editor
              </button>
              <button
                style="padding:6px 14px;border-radius:var(--radius-sm);font-size:12px;cursor:pointer;border:1px solid var(--bg-overlay);background:var(--bg-surface);color:var(--text-secondary);transition:background .15s"
                onClick={() => bridge.showSkillInFolder(selected.path)}
              >
                <i class="fas fa-folder-open" /> Show in Explorer
              </button>
            </div>

            {selected.tools.length > 0 && (
              <>
                <div style="font-size:12px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:6px">Tools</div>
                <div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:16px">
                  {selected.tools.map(t => (
                    <span key={t} style="background:var(--bg-primary);border:1px solid var(--bg-surface);color:var(--text-secondary);padding:2px 8px;border-radius:var(--radius-xs);font-size:11px;font-family:monospace">{t}</span>
                  ))}
                </div>
              </>
            )}
            {selected.disallowedTools && selected.disallowedTools.length > 0 && (
              <>
                <div style="font-size:12px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:6px">Disallowed tools</div>
                <div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:16px">
                  {selected.disallowedTools.map(t => (
                    <span key={t} style="background:var(--bg-primary);border:1px solid var(--bg-tint-red-soft);color:var(--accent-red);padding:2px 8px;border-radius:var(--radius-xs);font-size:11px;font-family:monospace;text-decoration:line-through">{t}</span>
                  ))}
                </div>
              </>
            )}

            <div style="font-size:11px;color:var(--text-disabled);font-family:monospace;word-break:break-all;margin-bottom:12px">{selected.path}</div>

            <AgentBody path={selected.path} body={bodyCache.get(selected.path)} />
          </div>
        )}
      </ThreeColumnDetail>
    </ThreeColumnPanel>
  )
}
