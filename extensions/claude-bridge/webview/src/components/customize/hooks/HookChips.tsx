// Atomic chip primitives + the host-resolution helper used across the Hooks
// tab (active hook detail, library card, log row). Extracted from
// `HooksPanel.tsx` (Sprint 2 / Stage C, C10) — these were 6 tiny components
// jammed at the bottom of the 658-LOC monolith.

import type { JSX } from 'preact'

interface HookHandlerLike {
  type: string
  host?: string
}

export function EventChip({ event }: { event: string }): JSX.Element {
  const colorMap: Record<string, string> = {
    PreToolUse: 'var(--accent-primary)',
    PostToolUse: 'var(--accent-primary)',
    PostToolUseFailure: 'var(--accent-primary)',
    UserPromptSubmit: 'var(--accent-purple)',
    SessionStart: 'var(--accent-purple)',
    SessionEnd: 'var(--accent-purple)',
    Stop: 'var(--accent-purple)',
    StopFailure: 'var(--accent-red)',
    SubagentStart: 'var(--accent-yellow)',
    SubagentStop: 'var(--accent-yellow)',
    PreCompact: 'var(--accent-orange)',
    PostCompact: 'var(--accent-orange)',
    PermissionRequest: 'var(--accent-red)',
    PermissionDenied: 'var(--accent-red)',
    Notification: 'var(--accent-teal)',
    WorktreeCreate: 'var(--accent-green)',
    WorktreeRemove: 'var(--accent-green)',
    FileChanged: 'var(--accent-green)',
    CwdChanged: 'var(--accent-green)',
  }
  const c = colorMap[event] ?? 'var(--accent-primary)'
  return (
    <span style={`font-size:var(--fs-xs);padding:1px 8px;border-radius:999px;background:color-mix(in srgb, ${c} 14%, transparent);color:${c};font-weight:700;letter-spacing:0.02em`}>
      {event}
    </span>
  )
}

export function TypeChip({ type }: { type: string }): JSX.Element {
  const icons: Record<string, string> = { command: 'fa-terminal', prompt: 'fa-sparkles', agent: 'fa-robot', http: 'fa-globe' }
  return (
    <span style="display:inline-flex;align-items:center;gap:4px;font-size:var(--fs-xs);padding:1px 6px;border-radius:999px;background:var(--tint-muted-medium);color:var(--text-secondary);font-weight:600">
      <i class={`fas ${icons[type] ?? 'fa-cog'}`} /> {type}
    </span>
  )
}

export function HostChip({ host }: { host: 'local' | 'server' | 'http' | 'auto' }): JSX.Element {
  const conf: Record<string, { color: string; icon: string; label: string; tooltip: string }> = {
    local: { color: 'var(--accent-orange)', icon: 'fa-bolt', label: 'Local', tooltip: 'Runs on your local machine via Electron — has access to files, git, npm, IDE' },
    server: { color: 'var(--accent-primary)', icon: 'fa-server', label: 'Server', tooltip: 'Runs inside the bridge container — limited to JSON transformations and HTTP calls' },
    http: { color: 'var(--accent-green)', icon: 'fa-globe', label: 'HTTP', tooltip: 'POSTs to external URL' },
    auto: { color: 'var(--text-muted)', icon: 'fa-shuffle', label: 'Auto', tooltip: 'Bridge picks default based on event/matcher' },
  }
  const c = conf[host] ?? conf.auto!
  return (
    <span data-tooltip={c.tooltip} style={`display:inline-flex;align-items:center;gap:4px;font-size:var(--fs-xs);padding:1px 6px;border-radius:999px;background:color-mix(in srgb, ${c.color} 14%, transparent);color:${c.color};font-weight:700`}>
      <i class={`fas ${c.icon}`} /> {c.label}
    </span>
  )
}

export function SourceChip({ source }: { source: { kind: string; pluginId?: string; projectPath?: string } }): JSX.Element {
  const map: Record<string, { color: string; icon: string; label: string }> = {
    user: { color: 'var(--accent-primary)', icon: 'fa-user', label: 'User' },
    project: { color: 'var(--accent-yellow)', icon: 'fa-folder-tree', label: 'Project' },
    plugin: { color: 'var(--accent-purple)', icon: 'fa-puzzle-piece', label: 'Plugin' },
    library: { color: 'var(--accent-teal)', icon: 'fa-box-archive', label: 'Library' },
    bridge: { color: 'var(--accent-rosewater)', icon: 'fa-bridge', label: 'Bridge' },
  }
  const c = map[source.kind] ?? map.user!
  const tooltip = source.pluginId ? `${c.label}: ${source.pluginId}` : source.projectPath ? `${c.label}: ${source.projectPath}` : c.label
  return (
    <span data-tooltip={tooltip} style={`display:inline-flex;align-items:center;gap:4px;font-size:var(--fs-xs);padding:1px 6px;border-radius:999px;background:color-mix(in srgb, ${c.color} 14%, transparent);color:${c.color};font-weight:600`}>
      <i class={`fas ${c.icon}`} /> {c.label}
    </span>
  )
}

export function CategoryChip({ category }: { category: string }): JSX.Element {
  return (
    <span style="font-size:var(--fs-xs);padding:1px 6px;border-radius:999px;background:var(--tint-muted-medium);color:var(--text-muted);font-weight:600;text-transform:uppercase;letter-spacing:0.04em">
      {category}
    </span>
  )
}

/** Resolve which host badge to display next to a hook in the Active / Library
 *  panes. Honours an explicit `host` field, otherwise falls back to type-based
 *  defaults (prompt/agent → server, http → http, command → auto). */
export function resolveDisplayHost(event: string, matcher: string | undefined, h: HookHandlerLike): 'local' | 'server' | 'http' | 'auto' {
  if (h.host === 'local' || h.host === 'server' || h.host === 'http') return h.host
  if (h.type === 'prompt' || h.type === 'agent') return 'server'
  if (h.type === 'http') return 'http'
  // Default: most events default to local for command type. Show 'auto' if no override.
  return 'auto'
}
