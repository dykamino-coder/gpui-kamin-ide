// Agent engine in the dashboard: one filter for every stats card, the label
// and badge of an engine, and the `engine` query parameter of the stats API.
// 'claude' is Claude Code CLI; 'codex' is ChatGPT (codex app-server).

import { engineFilter, setEngineFilter, type Engine, type EngineFilterValue } from '../../signals/engine'
export { engineFilter, setEngineFilter, engineParam, type Engine, type EngineFilterValue } from '../../signals/engine'

export const ENGINE_LABEL: Record<Engine, string> = { claude: 'Claude', codex: 'ChatGPT' }
const ENGINE_COLOR: Record<Engine, string> = { claude: 'var(--accent-orange)', codex: 'var(--accent-green)' }

export function asEngine(value: unknown): Engine {
  return value === 'codex' ? 'codex' : 'claude'
}

export function EngineFilterTabs() {
  const current = engineFilter.value
  const options: EngineFilterValue[] = ['all', 'claude', 'codex']
  return (
    <div role="group" aria-label="Engine" style={{ display: 'flex', gap: 2 }}>
      {options.map((v) => (
        <button
          key={v}
          type="button"
          aria-pressed={v === current}
          onClick={() => setEngineFilter(v)}
          title={v === 'all' ? 'All engines' : `Only ${ENGINE_LABEL[v]} chats`}
          style={{
            padding: '4px 8px',
            borderRadius: 4,
            border: 'none',
            cursor: 'pointer',
            background: v === current ? 'var(--bg-quaternary)' : 'transparent',
            color: v === current ? 'var(--text-primary)' : 'var(--text-muted)',
            fontSize: 12,
            fontWeight: 600,
          }}
        >
          {v === 'all' ? 'All' : ENGINE_LABEL[v]}
        </button>
      ))}
    </div>
  )
}

export function EngineBadge({ engine }: { engine: Engine }) {
  return (
    <span
      title={engine === 'codex' ? 'ChatGPT (codex)' : 'Claude Code'}
      style={{
        display: 'inline-block',
        padding: '1px 6px',
        borderRadius: 3,
        fontSize: 10,
        fontWeight: 700,
        letterSpacing: '0.03em',
        color: ENGINE_COLOR[engine],
        border: `1px solid color-mix(in srgb, ${ENGINE_COLOR[engine]} 55%, transparent)`,
        background: `color-mix(in srgb, ${ENGINE_COLOR[engine]} 10%, transparent)`,
        whiteSpace: 'nowrap',
      }}
    >
      {ENGINE_LABEL[engine]}
    </span>
  )
}
