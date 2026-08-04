// Right-column hook detail card extracted from ActiveHooks.tsx
// (Sprint 5 / Stage E1).

import type { JSX } from 'preact'
import { EventChip, TypeChip, HostChip, SourceChip, resolveDisplayHost } from './HookChips'

export interface FlatHookHandler {
  type: string
  command?: string
  prompt?: string
  url?: string
  host?: string
  if?: string
  timeout?: number
  shell?: string
}

export interface FlatHook {
  event: string
  matcherIdx: number
  matcher?: string
  handler: FlatHookHandler
  source: { kind: string; pluginId?: string; manifestPath?: string; projectPath?: string }
}

interface Props {
  fh: FlatHook
  editable: boolean
  showTest: boolean
  onToggleTest: () => void
  onEdit: () => void
  onDelete: () => void
}

function Field({ label, value, mono }: { label: string; value: string; mono?: boolean }): JSX.Element {
  return (
    <div style="display:grid;grid-template-columns:160px 1fr;gap:var(--space-3);align-items:baseline">
      <span style="font-size:var(--fs-sm);color:var(--text-secondary);font-weight:600">{label}</span>
      <span style={`font-size:var(--fs-base);color:var(--text-primary)${mono ? ';font-family:var(--font-mono);word-break:break-all' : ''}`}>{value}</span>
    </div>
  )
}

export function HookDetail({ fh, editable, showTest, onToggleTest, onEdit, onDelete }: Props): JSX.Element {
  const handler = fh.handler
  const summary = handler.command || handler.prompt || handler.url || '(no body)'
  const sourceLabel = fh.source.pluginId ? `Plugin: ${fh.source.pluginId}`
    : fh.source.projectPath ? `Project: ${fh.source.projectPath}`
    : fh.source.kind === 'user' ? 'User-global (~/.claude/settings.json)'
    : fh.source.kind === 'bridge' ? 'Bridge built-in'
    : fh.source.kind === 'library' ? 'Library template'
    : fh.source.kind

  return (
    <div style="display:flex;flex-direction:column;gap:var(--space-4);padding:var(--space-4) var(--space-5);height:100%;overflow-y:auto">
      <header style="display:flex;align-items:center;gap:var(--space-2);flex-wrap:wrap">
        <EventChip event={fh.event} />
        {fh.matcher && <span style="font-size:var(--fs-xs);padding:1px 8px;border-radius:999px;background:var(--tint-yellow-medium);color:var(--accent-yellow);font-family:var(--font-mono)">{fh.matcher}</span>}
        <TypeChip type={handler.type} />
        <HostChip host={resolveDisplayHost(fh.event, fh.matcher, handler) as any} />
        <SourceChip source={fh.source} />
      </header>

      <Field label="Source" value={sourceLabel} mono />
      {handler.shell && handler.type === 'command' && <Field label="Shell" value={handler.shell} />}
      <Field label="Timeout" value={`${handler.timeout ?? 600}s`} />
      {handler.if && <Field label="If condition" value={handler.if} mono />}
      {handler.host && handler.type === 'command' && <Field label="Where runs" value={handler.host} />}

      <div>
        <div style="font-size:var(--fs-sm);color:var(--text-secondary);font-weight:600;margin-bottom:6px">
          {handler.type === 'command' ? 'Command' : handler.type === 'prompt' || handler.type === 'agent' ? 'Prompt' : 'URL'}
        </div>
        <pre style="
          margin:0;background:var(--bg-base);border-radius:var(--radius-sm);
          padding:var(--space-3);font-family:var(--font-mono);font-size:var(--fs-sm);
          color:var(--text-primary);white-space:pre-wrap;word-break:break-word;
          border:1px solid var(--bg-surface);
        ">{summary}</pre>
      </div>

      <div style="display:flex;gap:var(--space-2)">
        <button type="button" onClick={onToggleTest} style={`padding:8px 16px;background:${showTest ? 'var(--accent-yellow)' : 'var(--bg-surface)'};color:${showTest ? 'var(--bg-mantle)' : 'var(--accent-yellow)'};border:none;border-radius:var(--radius-sm);cursor:pointer;font-weight:600`}>
          <i class={`fas ${showTest ? 'fa-chevron-up' : 'fa-play'}`} /> {showTest ? 'Hide test' : 'Test'}
        </button>
        <button type="button" onClick={onEdit} disabled={!editable}
          data-tooltip={editable ? '' : 'Read-only — comes from plugin/project. Edit the source file directly.'}
          style={`padding:8px 16px;background:${editable ? 'var(--accent-primary)' : 'var(--bg-surface)'};color:${editable ? 'var(--bg-mantle)' : 'var(--text-disabled)'};border:none;border-radius:var(--radius-sm);cursor:${editable ? 'pointer' : 'not-allowed'};font-weight:600`}>
          <i class="fas fa-pen" /> Edit
        </button>
        <button type="button" onClick={onDelete} disabled={!editable}
          data-tooltip={editable ? '' : 'Plugin/project hook — uninstall the plugin or edit its source file to remove.'}
          style={`margin-left:auto;padding:8px 16px;background:transparent;color:${editable ? 'var(--accent-red)' : 'var(--text-disabled)'};border:1px solid ${editable ? 'var(--accent-red)' : 'var(--bg-surface)'};border-radius:var(--radius-sm);cursor:${editable ? 'pointer' : 'not-allowed'};font-weight:600`}>
          <i class="fas fa-trash" /> Delete
        </button>
      </div>
    </div>
  )
}
