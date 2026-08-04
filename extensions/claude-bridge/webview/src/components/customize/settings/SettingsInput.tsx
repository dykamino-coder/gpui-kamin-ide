import type { JSX } from 'preact'
import styles from './SettingsInput.module.css'

interface Props {
  label: string
  type: string
  placeholder: string
  value: string
  onChange: (value: string) => void
  /** Optional inline badge shown next to the label (e.g. resolved token name). */
  labelHint?: string
}

export function SettingsInput({ label, type, placeholder, value, onChange, labelHint }: Props): JSX.Element {
  function handleKeyDown(e: KeyboardEvent): void {
    // Allow all keyboard shortcuts in inputs (Ctrl+X, Ctrl+C, Ctrl+V, Ctrl+A)
    e.stopPropagation()
  }

  return (
    <>
      <label class={styles.label}>
        {label}
        {labelHint && (
          <span style="margin-left:8px;padding:1px 6px;border-radius:var(--radius-xs);background:var(--bg-surface);color:var(--accent-green);font-size:10px;font-weight:500;letter-spacing:0.2px">
            {labelHint}
          </span>
        )}
      </label>
      <div style="position:relative">
        <input
          class={styles.input}
          type={type}
          placeholder={placeholder}
          value={value}
          onInput={(e) => onChange((e.target as HTMLInputElement).value)}
          onKeyDown={handleKeyDown}
          style={value ? 'padding-right:28px' : undefined}
        />
        {value && (
          <button
            onClick={() => onChange('')}
            style="position:absolute;right:6px;top:50%;transform:translateY(-50%);background:none;border:none;color:var(--text-disabled);cursor:pointer;font-size:11px;padding:2px 4px"
            data-tooltip="Clear"
            type="button"
          >
            <i class="fas fa-trash" />
          </button>
        )}
      </div>
    </>
  )
}
