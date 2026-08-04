// ============================================================================
// Shared formatting utilities — single source of truth
// ============================================================================

/** Format a date as `dd.mm.yy hh:mm:ss` */
export function formatDateTime(ts: Date | string): string {
  const d = ts instanceof Date ? ts : new Date(ts)
  if (isNaN(d.getTime())) return ''
  const dd = String(d.getDate()).padStart(2, '0')
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const yy = String(d.getFullYear()).slice(2)
  const hh = String(d.getHours()).padStart(2, '0')
  const min = String(d.getMinutes()).padStart(2, '0')
  const sec = String(d.getSeconds()).padStart(2, '0')
  return `${dd}.${mm}.${yy} ${hh}:${min}:${sec}`
}

/** Format a date as `dd.mm.yyyy hh:mm` (full year, no seconds) */
export function formatFullDate(ts: Date | string): string {
  const d = ts instanceof Date ? ts : new Date(ts)
  if (isNaN(d.getTime())) return ''
  const dd = String(d.getDate()).padStart(2, '0')
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const yyyy = d.getFullYear()
  const hh = String(d.getHours()).padStart(2, '0')
  const min = String(d.getMinutes()).padStart(2, '0')
  return `${dd}.${mm}.${yyyy} ${hh}:${min}`
}

/** Format a date with full year + locale time (for stat cards) */
export function formatLastAt(iso: string | undefined): string | undefined {
  if (!iso) return undefined
  const d = new Date(iso)
  if (isNaN(d.getTime())) return undefined
  const time = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  const dd = String(d.getDate()).padStart(2, '0')
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const yyyy = d.getFullYear()
  return `${dd}.${mm}.${yyyy} ${time}`
}

/** Relative time: "just now", "5m ago", "2h ago", "3d ago" */
export function formatRelativeTime(iso: string): string {
  const d = new Date(iso)
  if (isNaN(d.getTime())) return ''
  const sec = Math.floor((Date.now() - d.getTime()) / 1000)
  if (sec < 60) return 'just now'
  const min = Math.floor(sec / 60)
  if (min < 60) return `${min}m ago`
  const h = Math.floor(min / 60)
  if (h < 24) return `${h}h ago`
  const days = Math.floor(h / 24)
  return `${days}d ago`
}

/** Format token count: 1234 → "1.2K", 1234567 → "1.2M" */
export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

/** Shorten model name: "claude-3-5-sonnet-20241022" → "3-5-sonnet" */
export function formatModel(model: string): string {
  if (!model) return '\u2014'
  return model.replace('claude-', '').replace(/-\d{8}$/, '')
}

/** Format USD cost: ≥$1 → "$1.23", ≥$0.01 → "$0.05", >0 → "<$0.01", 0 → "$0" */
export function formatCost(cost: number): string {
  if (cost >= 0.01) return `$${cost.toFixed(2)}`
  if (cost > 0) return '<$0.01'
  return '$0'
}
