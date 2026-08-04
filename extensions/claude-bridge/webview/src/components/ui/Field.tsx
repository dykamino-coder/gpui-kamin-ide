import type { JSX, ComponentChildren } from 'preact'
import styles from './Field.module.css'

interface Props {
  label: string
  /** Renders a required marker after the label. */
  required?: boolean
  /** Help text under the input (hidden when `error` is set). */
  hint?: ComponentChildren
  /** Error text under the input — replaces the hint and colors red. */
  error?: ComponentChildren
  children: ComponentChildren
}

/** Label + required + hint/error scaffold around any input. One source of truth
 *  for form-field chrome so Settings, MCP and Hook forms stop each redefining it.
 *  The `*` marker is a visual affordance (aria-hidden) — required semantics ride
 *  on the wrapped input's own `required`/`aria-required` when the caller sets it. */
export function Field({ label, required, hint, error, children }: Props): JSX.Element {
  return (
    <div class={styles.field}>
      <label class={styles.label}>
        {label}{required ? <span class={styles.required} aria-hidden="true">*</span> : null}
      </label>
      {children}
      {error ? <span class={styles.error}>{error}</span> : hint ? <span class={styles.hint}>{hint}</span> : null}
    </div>
  )
}
