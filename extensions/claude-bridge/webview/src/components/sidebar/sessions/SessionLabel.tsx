import type { JSX } from 'preact'

interface SessionLabelProps {
  label: string
  hasName: boolean
  sessionTitle?: string
}

/** Plain label. Tinting is now done at the row level (SessionItem sets a
 *  `--tab-color` custom prop and a `tinted` class — see legacy-global.css). */
export function SessionLabel({ label, hasName, sessionTitle }: SessionLabelProps): JSX.Element {
  const tooltip = sessionTitle || label
  return (
    <span class={hasName ? 'session-label' : 'session-label unnamed'} data-tooltip={tooltip}>
      {label}
    </span>
  )
}
