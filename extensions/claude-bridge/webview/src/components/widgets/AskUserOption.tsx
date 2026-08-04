import { useSignal } from '@preact/signals'

interface AskUserOptionProps {
  label: string
  name: string
  value: number
  checked: boolean
  /** checkbox — для multiSelect (радио нельзя снять повторным кликом). */
  kind?: 'radio' | 'checkbox'
  onChange: () => void
}

/** A selectable option row rendered as a bordered card: transparent at rest,
 *  a soft surface on hover, and an accent border + tint when chosen — so the
 *  picked answer reads at a glance instead of relying on the tiny radio dot. */
export function AskUserOption({ label, name, value, checked, kind, onChange }: AskUserOptionProps) {
  const hovered = useSignal(false)
  const border = checked ? 'var(--accent-primary)' : hovered.value ? 'var(--bg-overlay)' : 'var(--bg-surface)'
  const background = checked
    ? 'color-mix(in srgb, var(--accent-primary) 14%, transparent)'
    : hovered.value ? 'var(--bg-surface)' : 'transparent'

  return (
    <label
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '10px',
        padding: '9px 12px',
        borderRadius: '10px',
        border: `1px solid ${border}`,
        background,
        cursor: 'pointer',
        transition: 'background 0.15s, border-color 0.15s',
        fontSize: '13px',
        fontWeight: checked ? 500 : 400,
        color: checked ? 'var(--text-primary)' : 'var(--text-secondary)',
      }}
      onMouseEnter={() => { hovered.value = true }}
      onMouseLeave={() => { hovered.value = false }}
    >
      <input
        type={kind ?? 'radio'}
        name={name}
        value={String(value)}
        checked={checked}
        onChange={onChange}
        style="accent-color:var(--accent-primary);width:15px;height:15px;flex-shrink:0;margin:0;"
      />
      <span>{label}</span>
    </label>
  )
}
