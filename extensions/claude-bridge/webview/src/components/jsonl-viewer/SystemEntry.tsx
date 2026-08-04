import type { JSX } from 'preact'
import type { JsonlEntryData } from '../../types/jsonl'
import { hasCliXmlTags } from './utils'
import { CliXmlContent } from './CliXmlContent'
import { JsonlTimestamp } from './JsonlTimestamp'

function formatNumber(n: number | undefined): string {
  if (n === undefined || n === null) return '—'
  return n.toLocaleString('en-US')
}

function CompactBoundary({ entry }: { entry: JsonlEntryData }): JSX.Element {
  // The full Pre-compact summary card always follows this boundary in
  // the stream — there's no need for a wordy explanation here. Render
  // a single dense divider line with the actual metrics so the user
  // sees "what happened, by how much" at a glance and the verbose
  // explanation lives in the summary card itself.
  const meta = entry.compactMetadata ?? {}
  // The CLI writes `preTokens`/`postTokens` (verified on real 2.1.x transcripts).
  // This card read `preCompactTokenCount`/`postCompactTokenCount` — names that
  // never existed in the data — so the before/after/reduction metrics were
  // ALWAYS undefined and the divider showed only "Conversation compacted".
  const pre = meta.preTokens
  const post = meta.postTokens
  const trigger = meta.trigger
  const reduction = pre && post ? Math.max(0, 100 - Math.round((post / pre) * 100)) : null
  const parts: JSX.Element[] = []
  if (trigger) parts.push(<span>trigger: <span style="color:var(--text-primary)">{trigger}</span></span>)
  if (pre !== undefined) parts.push(<span>before: <span style="color:var(--text-primary)">{formatNumber(pre)}</span></span>)
  if (post !== undefined) parts.push(<span>after: <span style="color:var(--text-primary)">{formatNumber(post)}</span></span>)
  if (reduction !== null) parts.push(<span style="color:var(--accent-green)">-{reduction}%</span>)

  return (
    <div
      class="jsonl-entry"
      style="background:transparent;border:none;border-radius:0;box-shadow:none;padding:6px 12px;margin:6px 40px 6px 40px;display:flex;align-items:center;gap:10px;color:var(--text-muted);font-size:11px"
    >
      <div style="flex:1;height:1px;background:linear-gradient(to right, transparent, var(--bg-overlay) 30%, var(--bg-overlay) 70%, transparent)" />
      <i class="fas fa-compress" style="font-size:10px;color:var(--text-secondary)" />
      <span style="color:var(--text-secondary);font-weight:600">Conversation compacted</span>
      {parts.length > 0 && (
        <span style="display:inline-flex;gap:10px;align-items:center;font-size:10px">
          {parts.map((p, i) => <span key={i} style="color:var(--text-muted)">{p}</span>)}
        </span>
      )}
      {entry.timestamp && (
        <span style="font-size:10px;color:var(--text-disabled)"><JsonlTimestamp timestamp={entry.timestamp} /></span>
      )}
      <div style="flex:1;height:1px;background:linear-gradient(to left, transparent, var(--bg-overlay) 30%, var(--bg-overlay) 70%, transparent)" />
    </div>
  )
}

function ApiErrorBlock({ entry }: { entry: JsonlEntryData }): JSX.Element {
  const retryIn = typeof entry.retryInMs === 'number' ? Math.round(entry.retryInMs) : null
  const attempt = entry.retryAttempt
  const maxRetries = entry.maxRetries
  const cause = entry.cause ?? entry.error?.cause ?? {}
  const code = cause.code ?? entry.error?.type ?? 'error'
  const path = cause.path
  return (
    <div class="jsonl-entry jsonl-entry-system">
      <div class="entry-role">
        <i class="fas fa-triangle-exclamation" style="font-size:10px;color:var(--accent-red)" />
        {' '}
        <span style="color:var(--accent-red);font-weight:600">API error</span>
        {' '}
        {entry.timestamp && <JsonlTimestamp timestamp={entry.timestamp} />}
      </div>
      <div style="padding:6px 12px;background:var(--tint-red-soft-2);border:1px solid var(--tint-red-border);border-radius:var(--radius-sm);margin-top:4px;color:var(--text-subtext);font-size:11px;font-family:ui-monospace,Consolas,monospace;line-height:1.55">
        <span style="color:var(--accent-red)">{code}</span>
        {path && <span style="color:var(--text-muted)"> · {path}</span>}
        {retryIn !== null && (
          <span style="color:var(--text-secondary)"> · retry in {retryIn}ms{attempt ? ` (attempt ${attempt}${maxRetries ? `/${maxRetries}` : ''})` : ''}</span>
        )}
      </div>
    </div>
  )
}

function AwaySummaryBlock({ entry }: { entry: JsonlEntryData }): JSX.Element | null {
  const content = entry.content ?? ''
  if (!content.trim()) return null
  return (
    <div class="jsonl-entry jsonl-entry-system">
      <div class="entry-role">
        <i class="fas fa-moon" style="font-size:10px;color:var(--accent-primary)" />
        {' '}
        <span style="color:var(--accent-primary);font-weight:600">Away summary</span>
        {' '}
        {entry.timestamp && <JsonlTimestamp timestamp={entry.timestamp} />}
      </div>
      <div style="padding:8px 12px;background:var(--bg-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);margin-top:4px;color:var(--text-primary);font-size:12px;line-height:1.55;white-space:pre-wrap">{content}</div>
    </div>
  )
}

function ModelFallbackBlock({ entry }: { entry: JsonlEntryData }): JSX.Element {
  // The CLI dropped to a lighter model mid-session (load/quota). `content` is
  // already a human line ("Switched to Sonnet 5 due to high demand …"); the two
  // ids give the exact transition. A dedicated warning strip so it reads as "the
  // model changed under you", not lost in the grey system-line noise.
  const from = entry.originalModel
  const to = entry.fallbackModel
  const content = entry.content ?? ''
  return (
    <div class="jsonl-entry jsonl-entry-system">
      <div class="entry-role">
        <i class="fas fa-shuffle" style="font-size:10px;color:var(--accent-yellow)" />
        {' '}
        <span style="color:var(--accent-yellow);font-weight:600">Model fallback</span>
        {' '}
        {entry.timestamp && <JsonlTimestamp timestamp={entry.timestamp} />}
      </div>
      <div style="padding:6px 12px;background:var(--tint-yellow-soft-2, var(--bg-primary));border:1px solid var(--tint-yellow-border, var(--bg-surface));border-radius:var(--radius-sm);margin-top:4px;color:var(--text-subtext);font-size:11px;line-height:1.55">
        {content && <div style="color:var(--text-primary)">{content}</div>}
        {(from || to) && (
          <div style="margin-top:2px;font-family:ui-monospace,Consolas,monospace;color:var(--text-muted)">
            {from ?? '?'} <span style="color:var(--accent-yellow)">→</span> {to ?? '?'}
          </div>
        )}
      </div>
    </div>
  )
}

export function SystemEntry({ entry }: { entry: JsonlEntryData }): JSX.Element | null {
  if (entry.subtype === 'compact_boundary') return <CompactBoundary entry={entry} />
  if (entry.subtype === 'api_error') return <ApiErrorBlock entry={entry} />
  if (entry.subtype === 'away_summary') return <AwaySummaryBlock entry={entry} />
  if (entry.subtype === 'model_fallback') return <ModelFallbackBlock entry={entry} />
  // turn_duration / other meta subtypes are CLI timing bookkeeping — hide.
  if (entry.subtype === 'turn_duration') return null
  // CLI ≥2.1.219: init-событие несёт mcp_server_errors — MCP из конфига,
  // которые НЕ поднялись. Раньше это глоталось молча («сервер просто не
  // отвечает»); теперь ошибки видны прямо в чате.
  if (entry.subtype === 'init') {
    const errs = (entry as { mcp_server_errors?: { name?: string; error?: string }[] }).mcp_server_errors
    if (!errs || errs.length === 0) return null
    return (
      <div class="jsonl-entry" style="margin:6px 0;padding:8px 12px;border-left:2px solid var(--accent-red);background:color-mix(in srgb, var(--accent-red) 8%, transparent);border-radius:6px;font-size:12px">
        <div style="color:var(--accent-red);font-weight:600;margin-bottom:4px">MCP серверы не поднялись</div>
        {errs.map((e, i) => (
          <div key={i} style="color:var(--text-secondary)">
            {e.name || '?'}: {e.error || 'unknown error'}
          </div>
        ))}
      </div>
    )
  }

  const content = entry.content || ''
  if (!content.trim()) return null

  const subtypeLabel = entry.subtype ? ` (${entry.subtype})` : ''

  return (
    <div class="jsonl-entry jsonl-entry-system">
      <div class="entry-role">
        <i class="fas fa-gear" style="font-size:10px;color:var(--text-disabled)" />
        {' '}
        <span style="color:var(--text-disabled)">System{subtypeLabel}</span>
        {' '}
        {entry.timestamp && <JsonlTimestamp timestamp={entry.timestamp} />}
      </div>
      {hasCliXmlTags(content)
        ? <CliXmlContent text={content} />
        : <div class="entry-text" style="color:var(--text-muted)">{content}</div>
      }
    </div>
  )
}
