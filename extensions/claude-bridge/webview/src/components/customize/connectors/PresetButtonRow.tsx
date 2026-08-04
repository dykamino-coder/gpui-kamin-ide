// Preset chip row extracted from AddServerForm.tsx (Sprint 5 / Stage E1).
// Hides chips whose preset name collides with an already-installed
// connector — caller passes that filter in via `presets`.

import type { JSX } from 'preact'
import type { PresetDefinition } from './add-server-presets'

interface Props {
  presets: PresetDefinition[]
  onApply: (preset: PresetDefinition) => void
}

export function PresetButtonRow({ presets, onApply }: Props): JSX.Element | null {
  if (presets.length === 0) return null
  return (
    <div style="margin-bottom:12px;display:flex;gap:8px;align-items:center;flex-wrap:wrap">
      <span style="font-size:11px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px">Presets:</span>
      {presets.map(p => (
        <button
          key={p.key}
          type="button"
          onClick={() => onApply(p)}
          style={`padding:4px 10px;border-radius:var(--radius-sm);font-size:11px;cursor:pointer;border:1px solid var(--bg-surface);background:var(--bg-mantle);color:${p.accent};font-weight:500`}
          data-tooltip={`${p.label} preset — one click to fill all fields`}
        >
          <i class={p.icon} style="margin-right:4px" />{p.label}
        </button>
      ))}
    </div>
  )
}
